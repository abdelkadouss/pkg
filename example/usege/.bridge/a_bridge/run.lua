---@diagnostic disable: undefined-global
return {
  supported_pkg_type = { 'single_executable' },
  -- just_a_dep = true, -- don't install if nothign depand on
  inastll = function(input, opts)
    -- do some thing ...
    local result = some_call(opts.some_opt)
    -- ...
    return {
      {
        path = 'out/bin',
        type = 'single_executable',
        version = result.versoion,
      },
    }
  end
  -- remove and update are optoanl, use default if not definde.
}
