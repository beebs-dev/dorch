import os
import subprocess
import shutil
from typing import List


def process_zip(zip_path: str, extract_dir: str):
    if not os.path.exists(zip_path):
        print(f"Zip file {zip_path} does not exist. Skipping.")
        return
    shutil.rmtree(extract_dir, ignore_errors=True)
    os.makedirs(extract_dir, exist_ok=True)
    print(f"Extracting {zip_path} to {extract_dir}...")
    subprocess.run(
        ["unzip", "-q", zip_path, "-d", extract_dir],
        check=True,
    )


def extract_wad_archive(out_dirs: List[str]):
    start_range = int(os.getenv("START_RANGE", "0"))
    end_range = int(os.getenv("END_RANGE", "256"))
    print(
        f"Ensuring chunks from {start_range:02x} to {end_range-1:02x} exist")
    missing = 0
    for i in range(start_range, end_range):
        found = False
        for out_dir in out_dirs:
            if os.path.exists(os.path.join(out_dir, f"{i:02x}")):
                found = True
                break
        if not found:
            missing += 1
            print(f"Chunk {i:02x} is missing in all output directories.")
    print(f"Total missing chunks: {missing}")

if __name__ == "__main__":
    extract_wad_archive(['/home/thavlik/Repositories/wads_extra',
                        '/media/spare/wads'])
