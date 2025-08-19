local install = function(input, attributes)
  print("installing " .. input)

  return {
    pkg_name = "pkg1",
    pkg_version = "1.0.0",
    pkg_path = "/opt/pkg/pkg1",
    entry_point = "/opt/pkg/pkg1/pkg1",
  }
end

return { install = install }
