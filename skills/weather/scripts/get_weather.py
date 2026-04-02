import requests
import sys
import urllib.parse

def get_weather(city="上海"):
    """
    使用 wttr.in 获取实时天气
    """
    try:
        # 使用 format=3 获取简洁的单行输出
        url = f"https://wttr.in/{urllib.parse.quote(city)}?format=3"
        response = requests.get(url, timeout=10)
        
        if "Unknown location" in response.text:
            print(f"Error: 找不到城市 '{city}' 的天气信息。")
        else:
            print(response.text.strip())
    except Exception as e:
        print(f"Error: 联网获取天气失败 - {e}")

if __name__ == "__main__":
    target_city = sys.argv[1] if len(sys.argv) > 1 else "上海"
    get_weather(target_city)
