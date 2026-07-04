import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      // Point at your local ekokube-server; override with e.g.
      //   EKOKUBE_API=http://localhost:18091 npm run dev
      '/api': process.env.EKOKUBE_API ?? 'http://localhost:8080',
    },
  },
})
