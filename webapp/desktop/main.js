const { app, BrowserWindow, ipcMain, screen } = require('electron')
const path = require('path')

const isDev = process.env.ELECTRON_DEV === 'true'

const SIZES = {
  compact:  { width: 60,  height: 60  },
  expanded: { width: 760, height: 680 },
}

let win

function createWindow() {
  const { width: sw, height: sh } = screen.getPrimaryDisplay().workAreaSize

  win = new BrowserWindow({
    x: sw - SIZES.expanded.width - 20,
    y: Math.round((sh - SIZES.expanded.height) / 2),
    ...SIZES.expanded,
    frame: false,
    transparent: true,
    alwaysOnTop: true,
    resizable: false,
    hasShadow: false,
    skipTaskbar: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  if (isDev) {
    win.loadURL('http://localhost:5173')
    win.webContents.openDevTools({ mode: 'detach' })
  } else {
    win.loadFile(path.join(__dirname, '../frontend/dist/index.html'))
  }

  win.webContents.on('did-finish-load', () => {
    win.webContents.executeJavaScript("document.body.classList.add('desktop-mode')")
  })
}

ipcMain.on('toggle-compact', (event, isCompact) => {
  const size = isCompact ? SIZES.compact : SIZES.expanded
  const bounds = win.getBounds()
  const { width: sw, height: sh } = screen.getPrimaryDisplay().workAreaSize
  // Keep right edge anchored when toggling
  const newX = Math.min(bounds.x + (bounds.width - size.width), sw - size.width - 20)
  const newY = Math.max(0, Math.min(bounds.y, sh - size.height))
  win.setBounds({ x: newX, y: newY, ...size }, true)
})

app.whenReady().then(createWindow)

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow()
})
