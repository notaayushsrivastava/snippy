import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@snippy/ui/src/tokens/primitives.css';
import '@snippy/ui/src/tokens/semantic.css';
import '@snippy/ui/src/tokens/component.css';
import { App } from './App.js';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
