return {
  out = '~/.local/share/nushell/plugins',
  link = { enable = false },
  version_track = true,
  path_type = 'file',       -- opts: file, executable, dir, executable_or_dir, file_or_dir, any
  install_as_assets = false -- can't be install with other pkgs
}
