@echo off
cd /d "%~dp0"
python -m pip install --quiet --user -r requirements.txt
python mares_apneist.py leggi %*
pause
