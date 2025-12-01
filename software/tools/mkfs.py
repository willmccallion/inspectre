#!/usr/bin/env python3
import struct
import os
import sys

# Configuration
KERNEL_SIZE = 65536
OUTPUT_IMG = "disk.img"
# Path to the kernel binary (Relative to software/)
KERNEL_BIN = "bin/kernel/kernel.bin"

# Directories to scan for apps (Relative to software/)
APP_DIRS = ["bin/user"]

def main():
    # Check if kernel exists
    if not os.path.exists(KERNEL_BIN):
        print(f"Error: {KERNEL_BIN} not found. Build kernel first.")
        sys.exit(1)

    # Read Kernel
    with open(KERNEL_BIN, "rb") as f:
        kernel_data = f.read()

    # Pad Kernel
    if len(kernel_data) > KERNEL_SIZE:
        print(f"Warning: Kernel too big ({len(kernel_data)} > {KERNEL_SIZE}). Truncating.")
        kernel_data = kernel_data[:KERNEL_SIZE]

    padding = b'\x00' * (KERNEL_SIZE - len(kernel_data))
    disk_data = bytearray(kernel_data + padding)

    files = []
    
    # Scan directories for .bin files
    for d in APP_DIRS:
        if os.path.exists(d):
            for fname in os.listdir(d):
                if fname.endswith(".bin"):
                    path = os.path.join(d, fname)
                    with open(path, "rb") as f:
                        content = f.read()
                        # Truncate name to 31 chars + null terminator
                        name_bytes = fname.replace(".bin", "").encode('utf-8')[:31]
                        name_bytes += b'\x00' * (32 - len(name_bytes))
                        files.append({
                            "name": name_bytes,
                            "content": content,
                            "size": len(content)
                        })

    file_count = len(files)
    disk_data.extend(struct.pack("<I", file_count))

    header_size = 40
    data_offset = KERNEL_SIZE + 4 + (file_count * header_size)

    # Write Headers
    for file in files:
        disk_data.extend(file["name"])
        disk_data.extend(struct.pack("<I", data_offset))
        disk_data.extend(struct.pack("<I", file["size"]))
        data_offset += file["size"]

    # Write Content
    for file in files:
        disk_data.extend(file["content"])

    # Save to disk.img
    with open(OUTPUT_IMG, "wb") as f:
        f.write(disk_data)

    print(f"Success: Created {OUTPUT_IMG} with Kernel + {file_count} apps.")

if __name__ == "__main__":
    main()
