## Release process
Releases of vl-convert are handled using [cargo-workspaces](https://github.com/pksunkara/cargo-workspaces), which can be installed with:

```
$ cargo install cargo-workspaces
```

## Tagging a new version
Check out the main branch

Bump version to the next minor rc.

```
$ cargo ws version preminor --all --pre-id rc
```

Bump version to the next minor.

```
$ cargo ws version minor --all
```


Bump version to the next patch.

```
$ cargo ws version patch --all
```