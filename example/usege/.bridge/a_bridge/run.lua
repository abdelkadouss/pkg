---@diagnostic disable: undefined-global
return {
  featurs_support = {
    pkg_type = { 'single_executable' },
    opts = { 'locked' },
    specify_version = true,
    hooks = {
      after = {
        install = true,
        update = true,
        remove = true
      },
      before = { remove = true }
    }
  },
  -- just_a_dep = true, -- don't install if nothign depand on
  inastll = function(input, version, opts)
    -- do some thing ...
    -- local result = some_call(opts.some_opt)
    -- ...
    return {
      {
        path = 'out/bin',
        type = 'single_executable',
        version = 'x.x.x',
      },
    }
  end
  -- remove and update are optoanl, use default if not definde.
}
