import requests
import os
import subprocess
import shutil
import sys
import time

BASE_URL = 'https://archive.org/download/wadarchive/DATA'


def _fmt_bytes(n: float) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    u = 0
    while n >= 1024 and u < len(units) - 1:
        n /= 1024.0
        u += 1
    if u == 0:
        return f"{int(n)}{units[u]}"
    return f"{n:.2f}{units[u]}"


def _fmt_rate(bps: float) -> str:
    return f"{_fmt_bytes(bps)}/s"


def _fmt_eta(seconds: float) -> str:
    if seconds == float("inf") or seconds != seconds:  # inf or NaN
        return "--:--"
    seconds = max(0, int(seconds))
    m, s = divmod(seconds, 60)
    h, m = divmod(m, 60)
    if h:
        return f"{h:d}:{m:02d}:{s:02d}"
    return f"{m:02d}:{s:02d}"


def download_zip(i: int, out_dir: str) -> str:
    filename = f"{i:02x}.zip"
    final_path = os.path.join(out_dir, filename)
    url = f"{BASE_URL}/{filename}"

    if os.path.exists(final_path):
        print(f"File {final_path} already exists. Skipping download.")
        return final_path

    os.makedirs(out_dir, exist_ok=True)
    tmp_path = os.path.join(out_dir, filename + ".part")

    print(f"Downloading {url} -> {tmp_path}")

    # Make requests a bit more robust and keep connections reused
    with requests.Session() as session:
        with session.get(url, stream=True, timeout=(10, 60)) as r:
            r.raise_for_status()

            total = r.headers.get("Content-Length")
            total_bytes = int(total) if total and total.isdigit() else None

            downloaded = 0
            start = time.monotonic()
            last_print = start
            last_bytes = 0

            # tune these if you want
            chunk_size = 1024 * 256  # 256KB
            min_print_interval = 0.15  # seconds

            with open(tmp_path, "wb") as f:
                for chunk in r.iter_content(chunk_size=chunk_size):
                    if not chunk:
                        continue
                    f.write(chunk)
                    downloaded += len(chunk)

                    now = time.monotonic()
                    if now - last_print >= min_print_interval:
                        elapsed = now - start
                        inst_bps = (downloaded - last_bytes) / max(now - last_print, 1e-9)
                        avg_bps = downloaded / max(elapsed, 1e-9)

                        if total_bytes:
                            pct = (downloaded / total_bytes) * 100.0
                            remaining = total_bytes - downloaded
                            eta = remaining / max(avg_bps, 1e-9)
                            line = (
                                f"\r{filename}  "
                                f"{pct:6.2f}%  "
                                f"{_fmt_bytes(downloaded)}/{_fmt_bytes(total_bytes)}  "
                                f"inst {_fmt_rate(inst_bps)}  avg {_fmt_rate(avg_bps)}  "
                                f"ETA {_fmt_eta(eta)}"
                            )
                        else:
                            line = (
                                f"\r{filename}  "
                                f"{_fmt_bytes(downloaded)}  "
                                f"inst {_fmt_rate(inst_bps)}  avg {_fmt_rate(avg_bps)}"
                            )
                        print(line, flush=True)
                        last_print = now
                        last_bytes = downloaded

            # final print line + newline
            end = time.monotonic()
            elapsed = end - start
            avg_bps = downloaded / max(elapsed, 1e-9)
            if total_bytes:
                sys.stdout.write(
                    f"\r{filename}  100.00%  "
                    f"{_fmt_bytes(downloaded)}/{_fmt_bytes(total_bytes)}  "
                    f"avg {_fmt_rate(avg_bps)}  ETA 00:00\n"
                )
            else:
                sys.stdout.write(f"\r{filename}  {_fmt_bytes(downloaded)}  avg {_fmt_rate(avg_bps)}\n")
            sys.stdout.flush()

    os.rename(tmp_path, final_path)
    print(f"Downloaded {final_path}")
    return final_path


def upload_files(dir: str,
                 bucket: str = "wadarchive",
                 endpoint: str = "https://nyc3.digitaloceanspaces.com"):
    print(f"Uploading files from {dir} to s3://{bucket}")
    subprocess.run(
        ["aws", "s3", "sync", dir, f"s3://{bucket}",
            "--endpoint", endpoint, "--acl", "public-read"],
        check=True,
    )
    print(f"Synced files from {dir} to s3://{bucket}")


def mark_done(done_path: str, zip_path: str):
    os.makedirs(os.path.dirname(done_path), exist_ok=True)
    with open(done_path, 'w') as f:
        f.write('1')
    print(f"Marked done: {done_path}")
    os.remove(zip_path)


def cleanup(zip_path: str, extract_dir: str):
    if os.path.exists(zip_path):
        os.remove(zip_path)
    if os.path.exists(extract_dir):
        shutil.rmtree(extract_dir)

def process_zip(i: int, out_dir: str):
    id = f"{i:02x}"
    zip_filename = f"{id}.zip"
    wad_unzip_dir = f"{out_dir}/wad_unzip"
    if not os.path.exists(wad_unzip_dir):
        os.makedirs(wad_unzip_dir)
    extract_dir = f"{wad_unzip_dir}/{id}"
    zip_path = os.path.join(out_dir, zip_filename)
    done_path = os.path.join(out_dir, f"_done", id)
    if os.path.exists(done_path):
        print(f"Skipping {id}, already done.")
        cleanup(zip_path, extract_dir)
        return
    if os.path.exists(extract_dir):
        upload_files(os.path.join(extract_dir, id))
        return mark_done(done_path, zip_path)
    tmp_dir = f"{extract_dir}_tmp"
    if os.path.exists(tmp_dir):
        shutil.rmtree(tmp_dir)
    os.makedirs(tmp_dir)
    zip_path = download_zip(i, out_dir)
    subprocess.run(
        ["unzip", "-q", zip_path, "-d", tmp_dir],
        check=True,
    )
    os.rename(tmp_dir, extract_dir)
    print(f"Unzipped {zip_path} to {extract_dir}")
    upload_files(os.path.join(extract_dir, id))
    mark_done(done_path, zip_path)
    cleanup(zip_path, extract_dir)

def download_wad_archive(out_dir: str):
    start_range = int(os.getenv("START_RANGE", "0"))
    end_range = int(os.getenv("END_RANGE", "256"))
    print(f"Processing WAD archive from {start_range} to {end_range}")
    for i in range(start_range, end_range):
        process_zip(i, out_dir)


if __name__ == "__main__":
    download_wad_archive('/data/wads')
