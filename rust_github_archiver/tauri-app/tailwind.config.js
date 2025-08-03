/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'lava-green': '#00ff41',
        'lava-yellow': '#ffff00',
        'lava-red': '#ff0041',
        'lava-orange': '#ff8c00',
      },
      animation: {
        'lava-flow': 'lava-flow 10s ease-in-out infinite',
        'bubble': 'bubble 8s ease-in-out infinite',
        'pulse-glow': 'pulse-glow 2s ease-in-out infinite',
      },
      keyframes: {
        'lava-flow': {
          '0%, 100%': { 
            transform: 'translateY(0px) scale(1)',
            borderRadius: '60% 40% 30% 70% / 60% 30% 70% 40%'
          },
          '50%': { 
            transform: 'translateY(-20px) scale(1.1)',
            borderRadius: '30% 60% 70% 40% / 50% 60% 30% 60%'
          },
        },
        'bubble': {
          '0%': { 
            transform: 'translateY(100vh) scale(0)',
            opacity: 0
          },
          '10%': {
            opacity: 1
          },
          '90%': {
            opacity: 1
          },
          '100%': { 
            transform: 'translateY(-20vh) scale(1)',
            opacity: 0
          },
        },
        'pulse-glow': {
          '0%, 100%': { 
            boxShadow: '0 0 20px rgba(0, 255, 65, 0.5)',
            opacity: 0.8
          },
          '50%': { 
            boxShadow: '0 0 40px rgba(0, 255, 65, 0.8)',
            opacity: 1
          },
        },
      },
      backdropBlur: {
        'xs': '2px',
      }
    },
  },
  plugins: [],
}
