import argparse
import os
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed

parser = argparse.ArgumentParser()
parser.add_argument("--mode", help="Defines which cairo files to compile")
parser.add_argument("--filter", help="Only process file paths containing this string")
parser.add_argument(
    "-j",
    "--jobs",
    type=int,
    default=os.cpu_count() or 1,
    help="Number of parallel jobs",
)
args = parser.parse_args()

subprocess.run(
    [
        "cargo",
        "build",
        "--package",
        "test-utils",
    ],
    check=True,
)


def get_dst_path(src_path):
    rel_path = os.path.relpath(src_path, start=src_root)
    dst_path = os.path.join(dst_root, rel_path)
    dst_path, ext = os.path.splitext(dst_path)

    dst_dir = os.path.dirname(dst_path)
    os.makedirs(dst_dir, exist_ok=True)

    return dst_path


def compile_cairo_project(src_path, binary_path):
    dst_path = get_dst_path(src_path)

    print(f"compiling project {src_path} into {dst_path}")
    subprocess.run(
        [
            binary_path,
            src_path,
            dst_path + ".sierra.json",
            dst_path + ".sierra",
        ],
        check=True,
    )


def compile_cairo_tests(src_path, starknet=False):
    dst_path = get_dst_path(src_path)

    print(f"compiling tests {src_path} into {dst_path}")
    cmd = [
        "target/debug/compile-cairo-tests",
        src_path,
        dst_path + ".tests.json",
    ]
    if starknet:
        cmd.append("--starknet")
    subprocess.run(cmd, check=True)


def compile_cairo_contract(src_path):
    dst_path = get_dst_path(src_path)

    print(f"compiling contract {src_path} into {dst_path}")
    subprocess.run(
        [
            "target/debug/compile-cairo-contract",
            src_path,
            dst_path + ".contract.json",
        ],
        check=True,
    )


def walk(subdir, f):
    for dirpath, dirnames, filenames in os.walk(
        os.path.join(src_root, subdir), followlinks=True
    ):
        if "cairo_project.toml" in filenames:
            if args.filter is None or args.filter in dirpath:
                f(dirpath)
            dirnames.clear()
        else:
            for filename in filenames:
                if filename.endswith(".cairo"):
                    filepath = os.path.join(dirpath, filename)
                    if args.filter is None or args.filter in filepath:
                        f(filepath)


tasks = []

if args.mode == "sierra-emu":
    src_root = "../../test_data"
    dst_root = "../../test_data_artifacts"
    walk(
        "programs/debug_utils",
        lambda p: tasks.append(
            lambda p=p: compile_cairo_project(
                p, "../../target/debug/compile-cairo-project"
            )
        ),
    )
else:
    src_root = "test_data"
    dst_root = "test_data_artifacts"
    walk(
        "tests",
        lambda p: tasks.append(lambda p=p: compile_cairo_tests(p, starknet=False)),
    )
    walk(
        "tests_starknet",
        lambda p: tasks.append(lambda p=p: compile_cairo_tests(p, starknet=True)),
    )
    walk(
        "programs",
        lambda p: tasks.append(
            lambda p=p: compile_cairo_project(p, "target/debug/compile-cairo-project")
        ),
    )
    walk(
        "contracts",
        lambda p: tasks.append(lambda p=p: compile_cairo_contract(p)),
    )

with ThreadPoolExecutor(max_workers=args.jobs) as executor:
    futures = [executor.submit(task) for task in tasks]
    for future in as_completed(futures):
        future.result()
