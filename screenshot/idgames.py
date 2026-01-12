import requests

HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (X11; Linux x86_64) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/120.0.0.0 Safari/537.36"
    ),
    "Accept": "application/json",
}

# https://www.doomworld.com/idgames/api/api.php?action=get&id=20316&out=json

def get_wad_info(id: int) -> dict:
    url = "https://www.doomworld.com/idgames/api/api.php"
    params = {
        "action": "get",
        "id": id,
        "out": "json",
    }
    response = requests.get(url, params=params, headers=HEADERS, timeout=10)
    response.raise_for_status()
    return response.json()


if __name__ == "__main__":
    wad_id = 20316
    wad_info = get_wad_info(wad_id)
    print(wad_info)