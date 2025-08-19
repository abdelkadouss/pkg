local install = function(input, opts)
  print("Installing via bridge1 useing the" .. input .. " input and the")
  for key, value in pairs(opts) do
    print(key .. " = " .. tostring(value))
  end

  return {
    pkg_name = "pkg1",
    pkg_version = "1.0.0",
    pkg_path = "/opt/pkg/pkg1",
    entry_point = "/opt/pkg/pkg1/pkg1",
  }
end

return { install = install }
