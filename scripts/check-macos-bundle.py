"""Reject bundles whose Mach-O deployment requirements exclude macOS 13.5."""
from pathlib import Path
import plistlib
import struct
import sys

SUPPORTED = (13, 5, 0)
ARM64 = 0x0100000C


def version(value):
    return (value >> 16, (value >> 8) & 255, value & 255)


def slices(data):
    magic = data[:4]
    if magic in (b"\xca\xfe\xba\xbe", b"\xca\xfe\xba\xbf"):
        count = struct.unpack_from(">I", data, 4)[0]
        wide = magic[-1] == 0xBF
        for i in range(count):
            entry = 8 + i * (32 if wide else 20)
            cpu = struct.unpack_from(">I", data, entry)[0]
            offset = struct.unpack_from(">Q" if wide else ">I", data, entry + 8)[0]
            yield from thin(data, offset, cpu)
    elif magic in (b"\xcf\xfa\xed\xfe", b"\xce\xfa\xed\xfe"):
        yield from thin(data, 0)


def thin(data, offset, cpu=None):
    magic = data[offset:offset + 4]
    assert magic in (b"\xcf\xfa\xed\xfe", b"\xce\xfa\xed\xfe"), "Unsupported Mach-O slice"
    cpu = cpu or struct.unpack_from("<I", data, offset + 4)[0]
    commands = struct.unpack_from("<I", data, offset + 16)[0]
    cursor = offset + (32 if magic[0] == 0xCF else 28)
    minimum = None
    for _ in range(commands):
        command, size = struct.unpack_from("<II", data, cursor)
        assert size >= 8
        if command == 0x32:  # LC_BUILD_VERSION
            minimum = version(struct.unpack_from("<I", data, cursor + 12)[0])
        elif command == 0x24:  # LC_VERSION_MIN_MACOSX
            minimum = version(struct.unpack_from("<I", data, cursor + 8)[0])
        cursor += size
    assert minimum is not None, "Mach-O has no deployment requirement"
    yield cpu, minimum


app = Path(sys.argv[1])
info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
assert tuple(map(int, info["LSMinimumSystemVersion"].split("."))) <= SUPPORTED
binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
assert ARM64 in {cpu for cpu, _ in slices(binary.read_bytes())}, "App has no native Apple Silicon slice"
checked = 0
for path in app.rglob("*"):
    if not path.is_file() or path.is_symlink():
        continue
    entries = list(slices(path.read_bytes()))
    for cpu, minimum in entries:
        assert minimum <= SUPPORTED, f"{path}: architecture {cpu:x} requires {minimum}, above macOS 13.5"
        print(f"{path.relative_to(app)} arch={cpu:x} minimum={minimum}")
        checked += 1
assert checked > 0
print(f"Validated {checked} Mach-O slices: compatible deployment target for macOS 13.5")
