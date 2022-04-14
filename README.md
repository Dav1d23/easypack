# Easypack: a simple, no-dependencies data packer/unpacker.

This crate provides an easy way to pack multiple files in a single one.
It can be useful to pack multiple read-only data in a single binary file,
for instance in case there are multiple small binary files that we need to
read together. Moreover, this is more convenient that asking the OS to open
a lot of small files, as overheads is reduced in here.

The main API is quite simple: one can create (pack) multiple data/files in one,
and read (unpack) them when needed. Note that while a Packer structure is
exposed, the related Unpacker is not since we internally allow multiple
versions to work. Thus, the API guarantees to always write the latest version,
and unpack all the supported versions.
