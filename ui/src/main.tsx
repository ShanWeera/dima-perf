/**
 * DiMA Desktop - Main Application Entry Point
 * 
 * Initializes the React application with providers and renders the root component.
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles/globals.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
