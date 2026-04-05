#!/usr/bin/env python3
"""
Extract sierra_code sections from cairo e2e test data files.

Input:  test_data/e2e_libfuncs_raw/  (files in //! > format)
Output: test_data/e2e_sierra/         (individual .sierra files)
"""

import os
import re
import shutil
import sys

SRC_DIR = "vendor/cairo/tests/e2e_test_data/libfuncs"
DST_DIR = "test_data/e2e_sierra"


def sanitize_name(name):
    """Convert test name to a safe filename."""
    return re.sub(r"[^\w\-]", "_", name.strip()).strip("_")


def parse_test_cases(text):
    """Parse //! > format into list of test-case dicts mapping section_key -> content."""
    cases = re.split(r"^//! > =+\s*$", text, flags=re.MULTILINE)
    results = []
    for case in cases:
        sections = {}
        section_order = []
        current_section = None
        lines = []
        for line in case.splitlines():
            m = re.match(r"^//! > (.+)$", line)
            if m:
                if current_section is not None:
                    sections[current_section] = "\n".join(lines).strip()
                current_section = m.group(1).strip()
                section_order.append(current_section)
                lines = []
            else:
                lines.append(line)
        if current_section is not None:
            sections[current_section] = "\n".join(lines).strip()
        if sections:
            sections["_order"] = section_order
            results.append(sections)
    return results


def main():
    if not os.path.isdir(SRC_DIR):
        print(
            f"Source directory {SRC_DIR} not found. "
            "Run 'git submodule update --init' first."
        )
        sys.exit(1)

    shutil.rmtree(DST_DIR, ignore_errors=True)

    count = 0
    for dirpath, _, filenames in sorted(os.walk(SRC_DIR)):
        for filename in sorted(filenames):
            filepath = os.path.join(dirpath, filename)

            with open(filepath, "r") as f:
                content = f.read()

            # Preserve subdirectory structure (e.g. starknet/syscalls)
            rel_dir = os.path.relpath(dirpath, SRC_DIR)
            test_cases = parse_test_cases(content)
            for case in test_cases:
                sierra = case.get("sierra_code", "").strip()
                if not sierra:
                    continue

                # The test name is the first section key (not a section called "test_name")
                order = case.get("_order", [])
                if not order:
                    continue
                test_name = order[0]

                safe_name = sanitize_name(test_name)
                if not safe_name:
                    continue

                out_dir = os.path.join(DST_DIR, rel_dir, filename)
                os.makedirs(out_dir, exist_ok=True)
                out_path = os.path.join(out_dir, f"{safe_name}.sierra")

                with open(out_path, "w") as f:
                    f.write(sierra + "\n")
                count += 1

    print(f"Extracted {count} sierra files into {DST_DIR}/")


if __name__ == "__main__":
    main()
