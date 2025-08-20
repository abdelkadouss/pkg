local install = function(input, opts)
  print("Installing via bridge2 useing the" .. input .. " input and the" .. table.concat(opts, ",") .. " opts")

  return {
    pkg_name = "pkg1",
    pkg_version = "1.0.0",
    pkg_path = "/opt/pkg/pkg1",
    entry_point = "/opt/pkg/pkg1/pkg1",
  }
end

Remove = function(input, opts)
  print("Removing via bridge2 useing the" .. input .. " input and the" .. table.concat(opts, ",") .. " opts")
end

return { install = install }
