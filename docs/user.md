# Bridges

u have to maek a dir for each bridge
each bridge dir should contain a `run` file
u have to hundle 3 commands
1. install - required, input: [ input: string ] # input from inputs files => output: pkg_path,pkg_version,pkg_entry_point(if pkg type is 'Directory'), env: the atributes that passed via inputs files
2. update - optional, input: [ input: string ] # input from inputs files => output: pkg_path,pkg_version,pkg_entry_point(if pkg type is 'Directory'), env: like atributes + the pkg_path
3. remove - optional, like update

## how to use the default impls (if u don't want to write the remove and update commands)
- write a small cammand called `remove` or `update` to the command the u want to use the default imples of
- print the string `__IMPL_DEFAULT` in the stderr
- then make the command feild with the exit code 1

# Notes

- make sure to make the `run` file executable, u can use `chmod +x run`
- make sure to add the run time in the top of the run file, example:

```nu
#!/usr/bin/env -S pkgx --quiet +nushell.sh nu@0.107.0
}
```

# Example

```nu
# for example the bridge is called b1
def "b1 install" [input: string] {
    $"this thing should be: ($input)" o> out
    return "./out,x.x.x"
}

def "b1 update" [input: string] {
    panic "this should make the update fail and print 'brdige return an error: <this message>'"
}

def "b1 remove" [input: string] {
    print -e "__IMPL_DEFAULT"
    exit 1 # this mean the app will use the default impls insha'Allah
}
```
