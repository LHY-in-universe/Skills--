const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('electronAPI', {
  isElectron: true,
  toggleCompact: (isCompact) => ipcRenderer.send('toggle-compact', isCompact),
})
