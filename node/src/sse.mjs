export async function* parseSse(readable) {
  if (!readable) {
    return;
  }

  const decoder = new TextDecoder();
  let buffer = "";

  for await (const chunk of readable) {
    buffer +=
      typeof chunk === "string"
        ? chunk
        : decoder.decode(chunk, { stream: true });
    buffer = buffer.replace(/\r\n?/g, "\n");

    let boundary;
    while ((boundary = buffer.indexOf("\n\n")) !== -1) {
      const block = buffer.slice(0, boundary);
      buffer = buffer.slice(boundary + 2);
      const parsed = parseEventBlock(block);
      if (parsed) {
        yield parsed;
      }
    }
  }

  buffer += decoder.decode();
  const trailing = parseEventBlock(buffer.trim());
  if (trailing) {
    yield trailing;
  }
}

function parseEventBlock(block) {
  if (!block) {
    return null;
  }

  let event = null;
  const data = [];

  for (const line of block.split("\n")) {
    if (!line || line.startsWith(":")) {
      continue;
    }
    if (line.startsWith("event:")) {
      event = line.slice(6).trim();
    } else if (line.startsWith("data:")) {
      data.push(line.slice(5).trimStart());
    }
  }

  if (data.length === 0) {
    return null;
  }

  return { event, data: data.join("\n") };
}

export function encodeSse(event) {
  const type = event?.type ?? "message";
  return `event: ${type}\ndata: ${JSON.stringify(event)}\n\n`;
}

export function encodeDone() {
  return "data: [DONE]\n\n";
}
