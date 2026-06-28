---@diagnostic disable: undefined-global
return {
  -- `out` not exeist means use default (define in .config)
  link = {
    enable = true,
    multi = true,
    required = true
  },
  version_track = true,
  path_type = 'dir',         -- opts: file, executable, dir, executable_or_dir, file_or_dir, any
  install_as_assets = false, -- can't be install with other pkgs
  hooks = {
    after_install = function(info)
      if not test.is_dir(info.path) then
        error 'givin path should be a dir'
      end
    end
  }
}
