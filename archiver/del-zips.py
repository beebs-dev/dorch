import os

def del_zip(zip_dir: str):
    start_range = 0
    end_range = 0xe7 #int(os.getenv("END_RANGE", "256"))
    print(
        f"Processing WAD archive from {start_range:02x} to {end_range-1:02x}")
    for i in range(start_range, end_range):
        zip_path = os.path.join(zip_dir, f"{i:02x}.zip")
        print(f"Deleting {zip_path}...")
        try:
            os.remove(zip_path)
        except FileNotFoundError:
            print(f"Zip file {zip_path} does not exist. Skipping.")


if __name__ == "__main__":
    del_zip('/home/thavlik/Repositories/wadarchive_zip')
