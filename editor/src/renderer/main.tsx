import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import NativePanelApp from './pages/NativePanelApp';
import './styles.css';

const nativePanel = new URLSearchParams(window.location.search).get('native-panel');

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {nativePanel ? <NativePanelApp panel={nativePanel as never} /> : <App />}
  </React.StrictMode>,
);
