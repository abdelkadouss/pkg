---@diagnostic disable: undefined-global
return {
  paths = {
    inputs = '~/.config/pkg',
    link = '/usr/local/pkg',
    bridges = '~/.config/pkg/.bridge',
    pkg_types_definition = '~/.config/pkg/.type',
    default_out = '/opt/pkg'
  },

  system = {
    database_path = '/var/db/pkg/packages.db',
    logs_path = '/var/log/pkg',
    max_thread_number = 10,
    clean_mode =
    "Soft" -- copts (config opts): [soft|strict] means it's send me wornings if strict is false or panic if strict is true if i have some unused pkg declaratoins.
  },
}
