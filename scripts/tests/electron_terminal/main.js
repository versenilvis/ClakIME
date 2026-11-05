const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const fs = require('fs');

let win;
const args = process.argv.slice(2);
let outputFile = '/tmp/electron_term.txt';
let readyFile = '/tmp/electron_term.ready';

let lagMs = 0;
for (const a of args) {
    if (a.endsWith('.txt') || a.endsWith('.bin')) outputFile = a;
    if (a.endsWith('.ready')) readyFile = a;
    if (a.startsWith('--lag=')) lagMs = parseInt(a.split('=')[1], 10) || 0;
}

app.whenReady().then(() => {
    win = new BrowserWindow({
        width: 1000,
        height: 600,
        title: 'clak_electron_xterm',
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false
        }
    });

    win.loadFile(path.join(__dirname, 'index.html'), {
        query: { out: outputFile, lag: lagMs }
    });

    ipcMain.on('ready', () => {
        fs.writeFileSync(readyFile, 'ready\n');
    });

    ipcMain.on('finish', () => {
        app.quit();
    });
});
