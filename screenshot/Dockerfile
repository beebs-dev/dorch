FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt .

RUN apt-get update \
 && apt-get install -y --no-install-recommends unzip \
 && rm -rf /var/lib/apt/lists/*

RUN pip install --no-cache-dir -r requirements.txt

COPY screenshot/ /app

CMD ["python", "/app/download_wad.py"]
