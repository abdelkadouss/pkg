---@diagnostic disable: undefined-global, redefined-local
return {
  -- TODO:
  -- config = { -- special felde to define some stuff about this input
  --   group = 2
  --   -- os = 'linux'
  --   -- optional = true -- don't insatll
  -- },

  a_bridge = {
    pkg1 = "some/repo",
    pkg2 = {
      input = "another/repo",
      deps = { 'pkg3' },
      version = '0.1.0',
      hook = {
        event = { 'install', 'update' }, -- means after insatll and update
        callback = function()
          -- do some thing
        end
      },
      opt = { locked = true }
    }
  },

  an_other_bridge = {
    pkg3 = {
      input = 'pkg-3',
      type = 'single_exec',
      just_a_dep = true, -- remove if nothing depend on or fiald to install what depend on.
      os = 'linux'       -- so anything depends on this gonna have this felde ether.
    }
  },

  -- TODO:
  -- custom = {
  --   pkg4 = {
  --     insatll = function()
  --       local result = sh.exec 'git clone protocal://host.domain/woner/repo';
  --       if result.success then
  --         local is_exec = test.is_exec 'repo/pkg'
  --         if not is_exec then error "it's not an executable" end
  --         fs.copy 'repo/pkg' 'some/where'
  --       end

  --       local result = net.fetch 'some/uri'
  --       local result = formats.json(result)

  --       return {
  --         {
  --           path = 'some/where',
  --           type = 'single_exec',
  --           version = result.versoion,
  --         }
  --       }
  --     end
  --   },

  --   pkg5 = {
  --     insatll = function()
  --       local make = require '.shared/build_systems/make'

  --       os.execute 'git clone protocal://host.domain/woner/repo'

  --       fs.cd 'repo'

  --       os.execute 'make'

  --       return {
  --         {
  --           path = 'out/bins',
  --           type = 'dir',
  --           version = result.versoion,
  --           link = {
  --             'bin1',
  --             'bin2'
  --           }
  --         },
  --         {
  --           path = 'out/lib',
  --           type = 'lib'
  --         }
  --       }
  --     end
  --   }
  -- }

}
