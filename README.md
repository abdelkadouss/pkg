                             بسم الله الرحمن الرحيم

# Pkg 📦 - A package manager

Pkg is a powerful insh'Allah, multi-language-secriptable, multi-platform, multi-repo yet simple and declarative package manager. In other sed u can insah'Allah insatll packages via pkg from any where insha'Allah in sync with a config files via simple scripts that wrotten in any language on any unix-like machine insha'Allah.

## Features
- 📦 Install packages from any repo
- 💻 Platform undependent (works insha'Allah on any unix-like machine)
- 🧱 written in rust (fast, portable yet simple insha'Allah)
- 📝 declarative all ur packages are wrtten in files (u have on ur machine what u have in the config files insha'Allah)
- 🚚 multi-repo support (u can write a simple script (bridge) to install from any where insha'Allah)
- 🗃️ full-management of packages insha'Allah install, update, remove support

# Installation
## Install from crates.io (soon insha'Allah)

todo

## From source

```bash
cargo install --git https://github.com/abdelkadous/pkg.git
```

> if you don't wanna shell completion disable the feature `cli_complation` by adding this flag: `--no-default-features`

# Usage

there is some consepts that u need to know before using pkg:

1. a **bridge** is a simple script that u can write to support declare new way to install packages from any where insha'Allah.
2. an **input** is the file where u write the packages that u want to install.
3. a **pkg** here is two types:
    1. a **Single Executable** this is the pkg just is a one binary u can run insah'Allah
    2. a **Directory** a pkg that is a directory that contains one then one file can be for e.g runtimes, libs, man pages, helper files, anthoer executable... but this type of pkg should have an entry point which a executable file to run the app from (pkg gonna link this file in PATH insha'Allah).

onec u know that let's dive into the usage process:

in the name of Allah:
## 1. Config pkg

the config is in `kdl` in this path: `~/.config/pkg/.config`
this is a config example with explanations:

```kdl
config {
  inputs { // here u declare the inputs of the program options
    path "~/.config/pkg" // where the program can find the inputs (the files where u write ur packages) insha'Allah
    bridges-set "~/.config/pkg/.bridges" // where the program can find the bridges (the install scripts) insha'Allah
  }
  output { // where the program write the outputs
    target-dir "/opt/pkg" // the dir where u wanna pkg to install the packages
    load-path "/usr/local/pkg" // this path is the only path that u have to add to PATH insha'Allah. which is a dir where pkg gonna make all the symlinks to the pkg (pkg entry points)
  }
  db {
    path "/var/db/pkg/packages.db" // pkg db path (a sqlite db that pkg used to store the packages info)
  }
}
```

> [!TIP]
> this is the recommended config file so we highly recommend to just copy and paste this. There is no default config u have to write this file or the program wont work insha'Allah

## 2. Add the bridges

the bridges as i said before is just scripts that contain the logic to install packages from any where insha'Allah.

u can write ur own bridges and for that read the [bridges section](#Bridges) to know the the rools insha'Allah but as bigging we recommend to start with using me bridges that u can find as my dotfiles at [here](https://github.com/abdelkadouss/dotfiles/tree/main/.config/pkg/.bridges). Just featch them and put them in the path `input.bridges-set` in the config file. And make sure to read the READMEs.

## 3. Add the inputs

finally u can add some pkgs to install. for e.g if u using my bridges u can add this write a file in the path u set as `input.path` in the config file called `test.kdl` then add this in the file:

```kdl
cargo { // cargo is the bridge name that u wanna use to install the packages inside the curly bracts
    bat "bat" // this is the first package for e.g, it's good to add a description to the pkg like so
    // NOTE: u see the pkg node is two parts, the first is the executable name (pkg gonna store the pkg executable after install with this name so u can reaname it BTW), the secand is called the input which what to pass to the bridge.
    // HACK: where the name == input u can use a shortcut by just writing the name, e.g: bat
    zoxide // like this
    // BUG: don't use the input shortcut if u have options (see #3)
}

eget { // this is another bridge to install from github, direct urls
    gh "cli/cli" // github cli
  nvim "neovim/neovim" keep_structure=#true target="*" entry_point="bin/nvim" // this is how to install nvim for e.g
    // NOTE: the extra args called options and it's gonna be passed to the bridge as env vars
}
```

## 4. Run pkg

Now u can install the packages by running pkg:

```bash
pkg build # or sync (sync == build)
```

now try remove the packages:

1. go ahead and remove any one from the inputs
e.g:

```diff
// test.kdl
cargo {
-   bat "bat" // ...
+   bat "bat" // ...
    ...
}
```

2. rerun the same command now:

```bash
pkg build # or sync
```

3. try using the command now (if u don't have it installed in anthoer place before alredy)

```bash
bat --version # the shll sould return error command not found
```

now try update the packages:

for all the pkgs just run:

```bash
pkg update
```

or for a specific pkg:

```bash
pkg update <the-pkg-name> # e.g: pkg update nvim
```

## 5. Full Example

for a full real example see the [examples](https://github.com/abdelkadouss/dotfiles/tree/main/.config/pkg) dir in my dotfiles repo.

## Bridges

### How to write a bridge ( how the bridges works )

a bridge is a dir in the path `input.bridges-set` in the config file. the dir should contain a file called `run` with the executable permistion, to make one run:

```bash
mkdir <path/to/bridges-set-dir>/test
touch <path/to/bridges-set-dir>/test/run
chmod +x <path/to/bridges-set-dir>/test/run
```

the `run` file should handle two args:
1. operation: string: install, update, remove
2. input: string: the string that u write after the pkg name in the inputs files, the inputs passed here one by one insha'Allah.

pkg gonna run ur bridges like this:

```bash
<path/to/bridges-set-dir>/test/run install <input> # or update, remove
```

u just have to handle one of the valid operations as must wich is the `install` operation. insha'Allah. but the other operations have a default implemention that u can use. To know how to use the default implemention and more info run the `pkg docs` command.

example of a bridge:

file tree:
```tree
// under the bridges-set dir
test_bridge
├── run
└── install_command_handler.sh
```

run:
```bash
#!/usr/bin/env bash

if [ "$1" == "install" ]; then
    ./install_command_handler.sh $2
else
    echo "__IMPL_DEFAULT" # tell pkg to use the default implemention
    exit 1 # it's should exit with 1 to let pkg know that he have to use the default implemention and this is not an unhanled error.
fi
```

install_command_handler.sh:
```bash
#!/usr/bin/env bash
echo 'this pkg should be' $1 > pkg
echo './pkg,0.0.1' # then u have to return the pkg path then comma then the pkg version. if this pkg type is dir so return pkg dir path then comma the version then comma then pkg executable (entry point). run pkg docs for more info.
```

# Known Issues

1. don't use the input shortcut if u have options (see [#3])
e.g:

```diff
bridge {
-   pkg opt1=val1 opt2=val2 opt3=val3 # ❌ this is wrong
+   pkg "pkg" opt1=val1 opt2=val2 opt3=val3 # ✅ this is fine insha'Allah
}
```

2. pkg has more then one executable but all of them should be linked in the PATH: pkg mainly new support pkg has more then one executable but it's just link one on the PATH.

# Contributing

the project is open to contributions, if u want to contribute open an issue or a pr.

# FAQ

1. Why shell completion is a feature?

    in my opinion u should't store static data in bin form specially if u need this data only onec, u ugly gonna run this command onec so why to store is the bin this is an overhead.

2. Why bridges are just an executable files what to do not add a scripting engine for better integration:

    look at issue [#2], and a short answer is this project was support lua before, but this change, and this is better for muli language support and a lot of stuff.

# License

Neda is licensed under the [GNU General Public License v3.0](https://github.com/aqhwan/neda/raw/refs/heads/main/LICENSE) plus one extra condition:

* Your use should be in line with the Islamic Sharia Laws. If there is any conflict between the terms of the 'GNU General Public License v3.0' and the Islamic Sharia Laws, the Islamic Sharia Laws shall prevail.
