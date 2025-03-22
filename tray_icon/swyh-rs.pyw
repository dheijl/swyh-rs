import subprocess
import time
import win32gui
import win32process
import win32con
import pystray
from PIL import Image
import sys

def start_program(target):
    """Startet das Programm und gibt das Prozessobjekt zurück."""
    process = subprocess.Popen(target)
    return process

def find_window_for_pid(pid):
    """Findet alle Fenster, die mit der übergebenen PID verknüpft sind."""
    result = []
    def callback(hwnd, _):
        nonlocal result
        ctid, cpid = win32process.GetWindowThreadProcessId(hwnd)
        if cpid == pid:
            result.append(hwnd)
    win32gui.EnumWindows(callback, None)
    return result

def minimize_to_tray(process):
    """Minimiert das Programm in den System Tray."""
    hwnds = find_window_for_pid(process.pid)
    for hwnd in hwnds:
        win32gui.ShowWindow(hwnd, win32con.SW_HIDE)

def show_program(process):
    """Zeigt das Programmfenster an."""
    hwnds = find_window_for_pid(process.pid)
    for hwnd in hwnds:
        win32gui.ShowWindow(hwnd, win32con.SW_SHOW)
        win32gui.SetForegroundWindow(hwnd)

def create_tray_icon(process):
    """Erstellt ein System Tray-Icon mit Menü."""
    image = Image.open("icon.png")  # Pfad zum Icon
    menu = pystray.Menu(
        pystray.MenuItem("Show", lambda: show_program(process)),
        pystray.MenuItem("Minimize", lambda: minimize_to_tray(process)),  # Minimieren-Button
        pystray.MenuItem("Exit", lambda: exit_program(icon, process))
    )
    icon = pystray.Icon("swyh-rs", image, "swyh-rs", menu)
    icon.run()
    return icon

def exit_program(icon, process):
    """Beendet das Programm und das Tray-Icon."""
    process.terminate()
    process.wait()
    icon.stop()
    sys.exit(0)  # Beendet das Python-Skript vollständig

if __name__ == "__main__":
    program = r'swyh-rs.exe'
    process = start_program(program)
    time.sleep(2)  # Kurze Verzögerung, um das Fenster anzuzeigen

    # Minimieren in den Tray
    minimize_to_tray(process)

    # Erstelle das Tray-Icon
    icon = create_tray_icon(process)
