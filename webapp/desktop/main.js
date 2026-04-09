const { app, BrowserWindow, ipcMain, screen } = require('electron')
const path = require('path')

const COMPACT  = { width: 64,  height: 64  }
const EXPANDED = { width: 420, height: 560 }
const MARGIN   = 16

let win

function createWindow() {
  const { width: sw, height: sh } = screen.getPrimaryDisplay().workAreaSize

  // Start in bottom-right corner, compact
  win = new BrowserWindow({
    x: sw - COMPACT.width  - MARGIN,
    y: sh - COMPACT.height - MARGIN,
    ...COMPACT,
    frame: false,
    transparent: true,
    alwaysOnTop: true,
    resizable: false,
    hasShadow: false,
    skipTaskbar: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  win.loadFile(path.join(__dirname, 'sprite.html'))
}

// Toggle between compact orb and expanded chat panel
ipcMain.on('toggle-compact', (_, isCompact) => {
  const { width: sw, height: sh } = screen.getPrimaryDisplay().workAreaSize

  if (isCompact) {
    win.setBounds({
      x: sw - COMPACT.width  - MARGIN,
      y: sh - COMPACT.height - MARGIN,
      ...COMPACT,
    }, true)
  } else {
    win.setBounds({
      x: sw - EXPANDED.width  - MARGIN,
      y: sh - EXPANDED.height - MARGIN,
      ...EXPANDED,
    }, true)
  }
})

app.whenReady().then(() => {
  app.dock?.hide()
  createWindow()
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow()
})
