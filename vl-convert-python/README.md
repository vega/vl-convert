Create development conda environment
```
$ conda create -n vl-convert-dev -c conda-forge python=3.10 maturin pytest black
```

Activate environment
```
$ conda activate vl-convert-dev
```

Change to Python package directory
```
$ cd vl-convert-python

```
Build Rust python package with maturin in develop mode
```
$ maturin develop --release
```

Run tests
```
$ pytest tests
```