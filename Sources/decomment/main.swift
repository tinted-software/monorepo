/// decomment — strip C-style /* ... */ comments from stdin to stdout.
/// Matches the behaviour of XNU's SETUP/decomment/decomment.c.
import Foundation

var input = FileHandle.standardInput.readDataToEndOfFile()
guard var text = String(data: input, encoding: .utf8) else {
  fputs("decomment: input is not valid UTF-8\n", stderr)
  exit(1)
}

var out = ""
out.reserveCapacity(text.count)

var i = text.startIndex
while i < text.endIndex {
  if text[i] == "/" && text.index(after: i) < text.endIndex {
    let next = text.index(after: i)
    if text[next] == "*" {
      // Block comment — skip until */
      var j = text.index(next, offsetBy: 1, limitedBy: text.endIndex) ?? text.endIndex
      while j < text.endIndex {
        if text[j] == "*" {
          let afterStar = text.index(after: j)
          if afterStar < text.endIndex && text[afterStar] == "/" {
            i = text.index(after: afterStar)
            break
          }
        }
        j = text.index(after: j)
      }
      if j >= text.endIndex { i = text.endIndex }
      continue
    } else if text[next] == "/" {
      // Line comment — skip to newline
      var j = text.index(next, offsetBy: 1, limitedBy: text.endIndex) ?? text.endIndex
      while j < text.endIndex && text[j] != "\n" {
        j = text.index(after: j)
      }
      i = j
      continue
    }
  }
  if text[i] == "\"" || text[i] == "'" {
    // String/char literal — copy verbatim including escapes
    let quote = text[i]
    out.append(text[i])
    i = text.index(after: i)
    while i < text.endIndex {
      if text[i] == "\\" && text.index(after: i) < text.endIndex {
        out.append(text[i])
        i = text.index(after: i)
        out.append(text[i])
        i = text.index(after: i)
        continue
      }
      out.append(text[i])
      let done = text[i] == quote
      i = text.index(after: i)
      if done { break }
    }
    continue
  }
  out.append(text[i])
  i = text.index(after: i)
}

print(out, terminator: "")
