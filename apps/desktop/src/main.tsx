import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@snippy/ui/src/tokens/primitives.css';
import '@snippy/ui/src/tokens/semantic.css';
import '@snippy/ui/src/tokens/component.css';
import { App } from './App.js';
import { ApprovalModal } from './ApprovalModal.js';
import { Overlay } from './Overlay.js';

const hash = window.location.hash;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {hash === '#overlay' ? <Overlay /> : hash === '#approval' ? <ApprovalModal /> : <App />}
  </StrictMode>,
);
