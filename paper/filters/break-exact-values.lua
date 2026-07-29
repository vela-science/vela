-- Preserve exact inline values while allowing TeX to wrap them.

function Code(element)
  local value = element.text
  if FORMAT:match("latex")
      and #value >= 28
      and not value:find("%s")
      and value:match("^[A-Za-z0-9_:/%.@%-]+$") then
    return pandoc.RawInline("latex", "\\nolinkurl{" .. value .. "}")
  end
  return element
end
