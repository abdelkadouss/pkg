# Usage
```bash
pkg sync # install every thing in config exapte the optanls pkg if exist. it's gonna panic config.system.clean_mod.strict = true
pkg sync --include-optionals # to include optional in to-install pkgs
pkg sync --include input # to include a spesific input
pkg sync --include input::pkg # to include a spesific pkg
pkg sync --execute-optionals # remove any thing optional
```
