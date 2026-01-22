import os
import subprocess
import shutil


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


def extract_wad_archive(zip_dir: str, out_dir: str):
    start_range = int(os.getenv("START_RANGE", "0"))
    end_range = int(os.getenv("END_RANGE", "256"))
    print(
        f"Processing WAD archive from {start_range:02x} to {end_range-1:02x}")
    for i in range(start_range, end_range):
        zip_path = os.path.join(zip_dir, f"{i:02x}.zip")
        extract_dir = os.path.join(out_dir, f"{i:02x}")
        process_zip(zip_path, extract_dir)


if __name__ == "__main__":
    extract_wad_archive('/home/thavlik/Repositories/wadarchive_zip',
                        '/media/spare/wads')
