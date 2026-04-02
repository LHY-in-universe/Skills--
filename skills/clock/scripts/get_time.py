import datetime
import time

def get_current_time():
    """
    获取系统当前的本地时间
    """
    now = datetime.datetime.now()
    # 格式化输出
    time_str = now.strftime('%Y-%m-%d %H:%M:%S')
    weekday = now.strftime('%A')
    
    # 转换为中文星期
    weekdays_cn = {
        'Monday': '星期一',
        'Tuesday': '星期二',
        'Wednesday': '星期三',
        'Thursday': '星期四',
        'Friday': '星期五',
        'Saturday': '星期六',
        'Sunday': '星期日'
    }
    
    print(f"🕒 当前本地时间: {time_str}")
    print(f"📅 {weekdays_cn.get(weekday, weekday)}")

if __name__ == "__main__":
    get_current_time()
