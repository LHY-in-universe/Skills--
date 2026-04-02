import collections
if not hasattr(collections, 'Callable'):
    import collections.abc
    collections.Callable = collections.abc.Callable

import psutil
import platform
import json
import time

def get_system_info():
    """
    获取本机系统信息（CPU、内存、运行时间、磁盘、网络）
    """
    try:
        # CPU 信息
        cpu_count = psutil.cpu_count(logical=True)
        cpu_load = psutil.getloadavg() # [1, 5, 15]

        # 内存信息
        mem = psutil.virtual_memory()
        
        # 磁盘信息 (根目录)
        disk = psutil.disk_usage('/')

        # 运行时间
        uptime_seconds = time.time() - psutil.boot_time()
        uptime_hours = round(uptime_seconds / 3600, 2)

        info = {
            "platform": platform.platform(),
            "arch": platform.machine(),
            "cpu_count": str(cpu_count) + " cores",
            "cpu_load_1_5_15": cpu_load,
            "memory": {
                "total": f"{round(mem.total / (1024**3), 2)} GB",
                "available": f"{round(mem.available / (1024**3), 2)} GB",
                "usage_percent": f"{mem.percent}%"
            },
            "disk": {
                "total": f"{round(disk.total / (1024**3), 2)} GB",
                "free": f"{round(disk.free / (1024**3), 2)} GB",
                "usage_percent": f"{disk.percent}%"
            },
            "uptime": f"{uptime_hours} hours",
            "hostname": platform.node()
        }

        print(json.dumps(info, indent=2, ensure_ascii=False))
    except Exception as e:
        error_info = {"error": f"获取系统信息失败: {str(e)}"}
        print(json.dumps(error_info, ensure_ascii=False))

if __name__ == "__main__":
    get_system_info()
