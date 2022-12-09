A FUSE FS that does on-the-fly sample type conversion.

Currently this is only a proof-of-concept that operates on a single file with
i16 values and converts to f32.

The f32 file view can be `memmap`ed (e.g. in numpy) without needing to convert the
entire file. This is useful for analysis of very large SDR capture files.

```bash
# Make a mount point
mkdir testmnt

# Mount the DSP FS
carge run -- test.cs16 testmnt &

# Check it out
ls -alu testmnt
hexdump testmnt/test.cs16

# Unmount when done
fusermount -u testmnt
```

Building fuser will require:

```
sudo apt-get install libfuse-dev pkg-config
```

See: https://github.com/cberner/fuser

# TODO

- Support more conversion types
- Provide conversions for an entire directory, not just a single file
- Other DSP operations? Resampling, filtering, etc.
