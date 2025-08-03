#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 🔥 GITHUB ARCHIVER - ELITE HACKER LAUNCHER v3.1337 🔥
# Professional Glitchy Terminal Experience with Advanced Animations
# ═══════════════════════════════════════════════════════════════════════════════
#
# Usage:
#   ./run.sh          - Full elite hacker experience with all animations
#   ./run.sh -s       - Silent mode, skip animations and launch setup directly
#   ./run.sh --skip   - Same as -s
#   ./run.sh -h       - Show help message
#   ./run.sh --help   - Same as -h
#
# ═══════════════════════════════════════════════════════════════════════════════

# Advanced Color Codes with RGB effects
declare -A COLORS=(
    [RESET]='\e[0m'
    [BOLD]='\e[1m'
    [DIM]='\e[2m'
    [UNDERLINE]='\e[4m'
    [BLINK]='\e[5m'
    [REVERSE]='\e[7m'
    [HIDDEN]='\e[8m'
    [STRIKETHROUGH]='\e[9m'
    
    # Standard Colors
    [BLACK]='\e[30m'
    [RED]='\e[31m'
    [GREEN]='\e[32m'
    [YELLOW]='\e[33m'
    [BLUE]='\e[34m'
    [MAGENTA]='\e[35m'
    [CYAN]='\e[36m'
    [WHITE]='\e[37m'
    
    # Bright Colors
    [BRIGHT_BLACK]='\e[90m'
    [BRIGHT_RED]='\e[91m'
    [BRIGHT_GREEN]='\e[92m'
    [BRIGHT_YELLOW]='\e[93m'
    [BRIGHT_BLUE]='\e[94m'
    [BRIGHT_MAGENTA]='\e[95m'
    [BRIGHT_CYAN]='\e[96m'
    [BRIGHT_WHITE]='\e[97m'
    
    # RGB Background Effects
    [BG_MATRIX]='\e[48;2;0;255;0m'
    [BG_FIRE]='\e[48;2;255;69;0m'
    [BG_CYBER]='\e[48;2;0;255;255m'
    [BG_PLASMA]='\e[48;2;255;0;255m'
    [BG_VOID]='\e[48;2;10;10;10m'
)

# Elite ASCII Characters
SKULL="💀"
FIRE="🔥"
LIGHTNING="⚡"
SKULL_CROSSBONES="☠️"
RADIOACTIVE="☢️"
BIOHAZARD="☣️"
GEAR="⚙️"
DIAMOND="💎"
EXPLOSION="💥"
ROCKET="🚀"
ALIEN="👽"
ROBOT="🤖"

# Terminal Control Functions
hide_cursor() { printf '\e[?25l'; }
show_cursor() { printf '\e[?25h'; }
clear_screen() { printf '\e[2J\e[H'; }
save_cursor() { printf '\e[s'; }
restore_cursor() { printf '\e[u'; }
move_cursor() { printf "\e[${1};${2}H"; }

# Advanced Glitch Effects
matrix_rain() {
    local duration=$1
    local start_time=$(date +%s)
    
    hide_cursor
    while [ $(($(date +%s) - start_time)) -lt $duration ]; do
        for ((i=1; i<=80; i++)); do
            local col=$((RANDOM % $(tput cols)))
            local row=$((RANDOM % $(tput lines)))
            local char=$(printf "\\$(printf %03o $((RANDOM % 94 + 33)))")
            local color=$((RANDOM % 6 + 31))
            
            move_cursor $row $col
            printf "\e[${color}m${char}\e[0m"
        done
        sleep 0.025
    done
    show_cursor
}

glitch_text() {
    local text="$1"
    
    # Safe glitch effect - just color flashing without character corruption
    for ((i=0; i<3; i++)); do
        # Cycle through different colors
        case $i in
            0) printf "\e[91;1m%s\e[0m" "$text" ;;  # Bright red
            1) printf "\r\e[95;1m%s\e[0m" "$text" ;;  # Bright magenta  
            2) printf "\r\e[93;1m%s\e[0m" "$text" ;;  # Bright yellow
        esac
        sleep 0.025
        if [ $i -lt 2 ]; then
            printf "\r%*s\r" ${#text} ""  # Clear for next iteration
        fi
    done
    printf "\n"
}

cyber_loading_bar() {
    local duration=$1
    local width=60
    local step_time=$(echo "scale=3; $duration / $width" | bc -l 2>/dev/null || echo "0.05")
    
    printf "${COLORS[BRIGHT_CYAN]}["
    for ((i=0; i<width; i++)); do
        local progress=$((i * 100 / width))
        
        if [ $((i % 10)) -eq 0 ]; then
            printf "${COLORS[BRIGHT_RED]}█${COLORS[RESET]}"
        elif [ $((i % 5)) -eq 0 ]; then
            printf "${COLORS[BRIGHT_YELLOW]}▓${COLORS[RESET]}"
        else
            printf "${COLORS[BRIGHT_GREEN]}░${COLORS[RESET]}"
        fi
        
        printf "${COLORS[BRIGHT_CYAN]}]${COLORS[RESET]} ${COLORS[BOLD]}%3d%%${COLORS[RESET]} " $progress
        
        # Add random glitch effects
        if [ $((RANDOM % 8)) -eq 0 ]; then
            printf "${COLORS[BLINK]}${COLORS[BRIGHT_RED]}${EXPLOSION}${COLORS[RESET]}"
            sleep 0.05
            printf "\b "
        fi
        
        sleep $step_time
        printf "\r${COLORS[BRIGHT_CYAN]}["
        for ((k=0; k<=i; k++)); do
            if [ $((k % 10)) -eq 0 ]; then
                printf "${COLORS[BRIGHT_RED]}█${COLORS[RESET]}"
            elif [ $((k % 5)) -eq 0 ]; then
                printf "${COLORS[BRIGHT_YELLOW]}▓${COLORS[RESET]}"
            else
                printf "${COLORS[BRIGHT_GREEN]}░${COLORS[RESET]}"
            fi
        done
    done
    printf "${COLORS[BRIGHT_CYAN]}]${COLORS[RESET]} ${COLORS[BOLD]}100%%${COLORS[RESET]} ${COLORS[BRIGHT_GREEN]}COMPLETE${COLORS[RESET]}\n"
}

plasma_wave() {
    local duration=$1
    local start_time=$(date +%s)
    local cols=$(tput cols)
    local center_col=$((cols / 2))
    local center_row=12
    
    hide_cursor
    clear_screen
    
    # Create a controlled plasma core effect
    while [ $(($(date +%s) - start_time)) -lt $duration ]; do
        # Draw expanding rings with proper trigonometry
        for ((radius=1; radius<=8; radius++)); do
            local color=$((31 + (radius % 6)))  # Cycle through colors
            
            # Draw expanding rings using angle calculations
            for ((angle=0; angle<360; angle+=45)); do
                # Simple approximation for circular positioning
                local offset_x=$((radius * 2))
                local offset_y=$((radius))
                
                # Calculate positions based on angle (simplified)
                local x=$((center_col + offset_x))
                local y=$((center_row + offset_y))
                
                if [ $x -gt 0 ] && [ $x -le $cols ] && [ $y -gt 0 ] && [ $y -le 24 ]; then
                    move_cursor $y $x
                    printf "\e[${color};1m●\e[0m"
                fi
                
                # Also draw on the opposite side
                x=$((center_col - offset_x))
                y=$((center_row - offset_y))
                if [ $x -gt 0 ] && [ $x -le $cols ] && [ $y -gt 0 ] && [ $y -le 24 ]; then
                    move_cursor $y $x
                    printf "\e[${color};1m●\e[0m"
                fi
            done
            
            # Add some energy particles
            for ((i=0; i<5; i++)); do
                local px=$((center_col + (RANDOM % 20) - 10))
                local py=$((center_row + (RANDOM % 6) - 3))
                if [ $px -gt 0 ] && [ $px -le $cols ] && [ $py -gt 0 ] && [ $py -le 24 ]; then
                    move_cursor $py $px
                    local particle_color=$((31 + RANDOM % 6))
                    printf "\e[${particle_color};1m⚡\e[0m"
                fi
            done
        done
        
        sleep 0.05
        clear_screen
    done
    
    show_cursor
    clear_screen
}

hacker_typewriter() {
    local text="$1"
    local color="$2"
    
    for ((i=0; i<=${#text}; i++)); do
        printf "${color}%s${COLORS[RESET]}" "${text:0:i}"
        
        # Random glitch chance
        if [ $((RANDOM % 15)) -eq 0 ]; then
            printf "${COLORS[BLINK]}${COLORS[BRIGHT_RED]}█${COLORS[RESET]}"
            sleep 0.05
            printf "\b "
        fi
        
        # Variable typing speed for realism
        local delay=$(echo "scale=3; 0.02 + ($RANDOM % 8) / 1000" | bc -l 2>/dev/null || echo "0.05")
        sleep $delay
        printf "\r"
    done
    printf "${color}%s${COLORS[RESET]}\n" "$text"
}

digital_rain() {
    local duration=$1
    local start_time=$(date +%s)
    local cols=$(tput cols)
    local -a drops
    local -a positions
    
    # Initialize drops
    for ((i=0; i<cols/4; i++)); do
        drops[i]=$((RANDOM % $(tput lines)))
        positions[i]=$((RANDOM % cols))
    done
    
    hide_cursor
    while [ $(($(date +%s) - start_time)) -lt $duration ]; do
        for ((i=0; i<${#drops[@]}; i++)); do
            local row=${drops[i]}
            local col=${positions[i]}
            
            # Clear old position
            if [ $row -gt 1 ]; then
                move_cursor $((row-1)) $col
                printf " "
            fi
            
            # Draw new drop
            move_cursor $row $col
            local char=$(printf "\\$(printf %03o $((RANDOM % 26 + 65)))")
            printf "${COLORS[BRIGHT_GREEN]}${char}${COLORS[RESET]}"
            
            # Update position
            drops[i]=$((row + 1))
            if [ ${drops[i]} -gt $(tput lines) ]; then
                drops[i]=1
                positions[i]=$((RANDOM % cols))
            fi
        done
        sleep 0.05
    done
    show_cursor
    clear_screen
}

elite_banner() {
    clear_screen
    
    # Get terminal dimensions
    local cols=$(tput cols)
    local lines=$(tput lines)
    
    # Determine logo size based on terminal dimensions
    # Large: 100+ cols, 35+ lines
    # Medium: 80+ cols, 25+ lines  
    # Tiny: anything smaller
    
    if [ $cols -ge 100 ] && [ $lines -ge 35 ]; then
        # LARGE LOGO - Full ASCII art
        cat << 'EOF'
    
    ██████╗ ██╗████████╗██╗  ██╗██╗   ██╗██████╗      █████╗ ██████╗  ██████╗██╗  ██╗██╗██╗   ██╗███████╗██████╗ 
    ██╔════╝██║╚══██╔══╝██║  ██║██║   ██║██╔══██╗    ██╔══██╗██╔══██╗██╔════╝██║  ██║██║██║   ██║██╔════╝██╔══██╗
    ██║  ███╗██║   ██║   ███████║██║   ██║██████╔╝    ███████║██████╔╝██║     ███████║██║██║   ██║█████╗  ██████╔╝
    ██║   ██║██║   ██║   ██╔══██║██║   ██║██╔══██╗    ██╔══██║██╔══██╗██║     ██╔══██║██║╚██╗ ██╔╝██╔══╝  ██╔══██╗
    ╚██████╔╝██║   ██║   ██║  ██║╚██████╔╝██████╔╝    ██║  ██║██║  ██║╚██████╗██║  ██║██║ ╚████╔╝ ███████╗██║  ██║
     ╚═════╝ ╚═╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ╚═════╝     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═╝
                                                                                                                   
EOF
        echo ""
        printf "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}"
        glitch_text "                          ${FIRE} E L I T E   H A C K E R   L A U N C H E R ${FIRE}"
        printf "${COLORS[RESET]}"
        echo ""
        
        printf "${COLORS[BRIGHT_RED]}${COLORS[BLINK]}"
        cat << 'EOF'
    ╔══════════════════════════════════════════════════════════════════════════════════════════════════════════╗
    ║  ⚠️  WARNING: UNAUTHORIZED ACCESS DETECTED ⚠️  INITIATING SECURITY PROTOCOLS ⚠️  STAND BY ⚠️        ║
    ╚══════════════════════════════════════════════════════════════════════════════════════════════════════════╝
EOF
        printf "${COLORS[RESET]}\n"
        
    elif [ $cols -ge 80 ] && [ $lines -ge 25 ]; then
        # MEDIUM LOGO - Compact ASCII art
        cat << 'EOF'

    ██████╗ ██╗████████╗██╗  ██╗██╗   ██╗██████╗      █████╗ ██████╗  ██████╗██╗  ██╗██╗██╗   ██╗███████╗██████╗ 
    ██╔════╝██║╚══██╔══╝██║  ██║██║   ██║██╔══██╗    ██╔══██╗██╔══██╗██╔════╝██║  ██║██║██║   ██║██╔════╝██╔══██╗
    ██║  ███╗██║   ██║   ███████║██║   ██║██████╔╝    ███████║██████╔╝██║     ███████║██║██║   ██║█████╗  ██████╔╝
    ╚██████╔╝██║   ██║   ██║  ██║╚██████╔╝██████╔╝    ██║  ██║██║  ██║╚██████╗██║  ██║██║ ╚████╔╝ ███████╗██║  ██║
     ╚═════╝ ╚═╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ╚═════╝     ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═══╝  ╚══════╝╚═╝  ╚═╝

EOF
        echo ""
        printf "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}"
        glitch_text "            ${FIRE} E L I T E   H A C K E R   L A U N C H E R ${FIRE}"
        printf "${COLORS[RESET]}"
        echo ""
        
        printf "${COLORS[BRIGHT_RED]}${COLORS[BLINK]}"
        cat << 'EOF'
    ╔════════════════════════════════════════════════════════════════════════════════════╗
    ║  ⚠️  WARNING: UNAUTHORIZED ACCESS ⚠️  SECURITY PROTOCOLS ⚠️  STAND BY ⚠️        ║
    ╚════════════════════════════════════════════════════════════════════════════════════╝
EOF
        printf "${COLORS[RESET]}\n"
        
    else
        # TINY LOGO - Text-based minimal
        echo ""
        printf "${COLORS[BRIGHT_CYAN]}${COLORS[BOLD]}"
        cat << 'EOF'
    ╔══════════════════════════════════════════════════════════╗
    ║                    GITHUB ARCHIVER                      ║
    ║                 🔥 ELITE HACKER LAUNCHER 🔥             ║
    ╚══════════════════════════════════════════════════════════╝
EOF
        printf "${COLORS[RESET]}\n"
        echo ""
        printf "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}"
        glitch_text "        ${SKULL_CROSSBONES} E L I T E   L A U N C H E R ${SKULL_CROSSBONES}"
        printf "${COLORS[RESET]}"
        echo ""
        
        printf "${COLORS[BRIGHT_YELLOW]}${COLORS[BLINK]}"
        cat << 'EOF'
    ╔════════════════════════════════════════════════════════╗
    ║  ⚠️  UNAUTHORIZED ACCESS ⚠️  STAND BY ⚠️              ║
    ╚════════════════════════════════════════════════════════╝
EOF
        printf "${COLORS[RESET]}\n"
    fi
}

system_breach_animation() {
    echo ""
    hacker_typewriter "${SKULL_CROSSBONES} INITIATING SYSTEM BREACH..." "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}"
    sleep 0.5
    
    echo ""
    printf "${COLORS[BRIGHT_YELLOW]}${COLORS[BOLD]}${RADIOACTIVE} SCANNING FOR VULNERABILITIES...${COLORS[RESET]}\n"
    cyber_loading_bar 2
    
    echo ""
    hacker_typewriter "${BIOHAZARD} EXPLOITING GITHUB API ENDPOINTS..." "${COLORS[BRIGHT_MAGENTA]}${COLORS[BOLD]}"
    sleep 0.8
    
    echo ""
    printf "${COLORS[BRIGHT_CYAN]}${COLORS[BOLD]}${GEAR} LOADING ADVANCED SCRAPING ALGORITHMS...${COLORS[RESET]}\n"
    cyber_loading_bar 2
    
    echo ""
    glitch_text "${EXPLOSION} BYPASSING RATE LIMITERS..."
    sleep 0.5
}

neural_network_effect() {
    local duration=$1
    local start_time=$(date +%s)
    local cols=$(tput cols)
    local lines=$(tput lines)
    
    hide_cursor
    while [ $(($(date +%s) - start_time)) -lt $duration ]; do
        # Draw neural connections
        for ((i=0; i<20; i++)); do
            local x1=$((RANDOM % cols))
            local y1=$((RANDOM % lines))
            local x2=$((RANDOM % cols))
            local y2=$((RANDOM % lines))
            
            # Draw connection line
            move_cursor $y1 $x1
            printf "${COLORS[BRIGHT_BLUE]}●${COLORS[RESET]}"
            move_cursor $y2 $x2
            printf "${COLORS[BRIGHT_CYAN]}◯${COLORS[RESET]}"
            
            # Simulate data flow
            move_cursor $((y1 + (y2-y1)/2)) $((x1 + (x2-x1)/2))
            printf "${COLORS[BRIGHT_YELLOW]}⚡${COLORS[RESET]}"
        done
        sleep 0.1
        clear_screen
    done
    show_cursor
}

quantum_tunnel_effect() {
    echo ""
    printf "${COLORS[BRIGHT_MAGENTA]}${COLORS[BOLD]}"
    cat << 'EOF'
    ████████████████████████████████████████████████████████████████████████████████
    ██                                                                            ██
    ██  🌌 QUANTUM TUNNEL ESTABLISHED 🌌 ENTERING CYBERSPACE DIMENSION 🌌        ██
    ██                                                                            ██
    ████████████████████████████████████████████████████████████████████████████████
EOF
    printf "${COLORS[RESET]}\n"
    
    # Spinning tunnel effect
    local frames=("◐" "◓" "◑" "◒")
    for ((i=0; i<20; i++)); do
        local frame=${frames[$((i % 4))]}
        printf "\r${COLORS[BRIGHT_CYAN]}${COLORS[BOLD]}    ${frame} TUNNELING THROUGH THE MATRIX ${frame} PROGRESS: %d%% ${frame}     " $((i * 5))
        sleep 0.1
    done
    echo ""
}

elite_system_info() {
    echo ""
    printf "${COLORS[BRIGHT_GREEN]}${COLORS[BOLD]}"
    cat << 'EOF'
    ╔═══════════════════════════════════════════════════════════════════════════════╗
    ║                           🔥 SYSTEM SPECIFICATIONS 🔥                        ║
    ╠═══════════════════════════════════════════════════════════════════════════════╣
EOF
    printf "${COLORS[RESET]}"
    
    printf "    ║ ${COLORS[BRIGHT_CYAN]}${GEAR} TARGET:${COLORS[RESET]}        GitHub Archive Infrastructure              ║\n"
    printf "    ║ ${COLORS[BRIGHT_YELLOW]}${LIGHTNING} PROTOCOL:${COLORS[RESET]}     Rust-Based Elite Scraping Engine         ║\n"
    printf "    ║ ${COLORS[BRIGHT_RED]}${FIRE} PAYLOAD:${COLORS[RESET]}       PostgreSQL + Advanced Analytics          ║\n"
    printf "    ║ ${COLORS[BRIGHT_MAGENTA]}${DIAMOND} STATUS:${COLORS[RESET]}        WEAPONS GRADE - AUTHORIZED PERSONNEL ONLY ║\n"
    printf "    ║ ${COLORS[BRIGHT_WHITE]}${ALIEN} VERSION:${COLORS[RESET]}       v3.1337 - QUANTUM ENHANCED               ║\n"
    
    printf "${COLORS[BRIGHT_GREEN]}${COLORS[BOLD]}"
    cat << 'EOF'
    ╚═══════════════════════════════════════════════════════════════════════════════╝
EOF
    printf "${COLORS[RESET]}\n"
}

final_breach_sequence() {
    echo ""
    printf "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}${COLORS[BLINK]}"
    cat << 'EOF'
    ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
    █ 🚨 CRITICAL ALERT 🚨 SYSTEM BREACH IN PROGRESS 🚨 ALL SYSTEMS COMPROMISED 🚨 █
    ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
EOF
    printf "${COLORS[RESET]}\n"
    
    sleep 1
    
    echo ""
    hacker_typewriter "${ROCKET} LAUNCHING GITHUB ARCHIVER ELITE INTERFACE..." "${COLORS[BRIGHT_GREEN]}${COLORS[BOLD]}"
    echo ""
    
    # Final countdown
    for countdown in {3..1}; do
        printf "${COLORS[BRIGHT_YELLOW]}${COLORS[BOLD]}${COLORS[BLINK]}    ${EXPLOSION} LAUNCH IN T-${countdown} ${EXPLOSION}    ${COLORS[RESET]}\r"
        sleep 1
    done
    
    echo ""
    printf "${COLORS[BRIGHT_GREEN]}${COLORS[BOLD]}${COLORS[BLINK]}    ${ROCKET} IGNITION SUCCESSFUL! ENTERING CYBER REALM... ${ROCKET}    ${COLORS[RESET]}\n"
    echo ""
    sleep 1
}

# ═══════════════════════════════════════════════════════════════════════════════
# 🔥 MAIN EXECUTION SEQUENCE 🔥
# ═══════════════════════════════════════════════════════════════════════════════

main() {
    # Check for help flag
    if [[ "$1" == "-h" ]] || [[ "$1" == "--help" ]]; then
        echo "🔥 GitHub Archiver - Elite Hacker Launcher v3.1337 🔥"
        echo ""
        echo "Usage:"
        echo "  ./run.sh          - Full elite hacker experience with all animations"
        echo "  ./run.sh -s       - Silent mode, skip animations and launch setup directly"
        echo "  ./run.sh --skip   - Same as -s"
        echo "  ./run.sh -h       - Show this help message"
        echo "  ./run.sh --help   - Same as -h"
        echo ""
        echo "This launcher provides an epic terminal experience before launching"
        echo "the GitHub Archiver setup interface. Use silent mode for quick access."
        return
    fi
    
    # Check for silent/skip mode
    if [[ "$1" == "-s" ]] || [[ "$1" == "--skip" ]]; then
        # Silent mode - skip all animations and go straight to setup
        echo "🚀 GitHub Archiver - Silent Mode"
        echo "Launching setup interface directly..."
        
        # Check if rust_github_archiver directory exists
        if [ ! -d "rust_github_archiver" ]; then
            printf "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}${SKULL} ERROR: rust_github_archiver directory not found!${COLORS[RESET]}\n"
            printf "${COLORS[BRIGHT_YELLOW]}Please ensure you're running this from the GitArchiver root directory.${COLORS[RESET]}\n"
            exit 1
        fi
        
        # Launch the setup script directly
        cd rust_github_archiver && ./setup.sh
        return
    fi
    
    # Check if running in terminal
    if [ ! -t 1 ]; then
        echo "ERROR: This script must be run in a terminal for full effects!"
        exit 1
    fi
    
    # Store original terminal state
    local original_cursor_state=$(tput cnorm 2>/dev/null || echo "")
    
    # Ensure we restore terminal on exit
    trap 'clear_screen; show_cursor; echo ""; echo "Terminal state restored."; exit 0' EXIT INT TERM
    
    # Phase 1: Elite Banner Display
    elite_banner
    sleep 2
    
    # Phase 2: Matrix Rain Effect
    echo ""
    hacker_typewriter "${ALIEN} ESTABLISHING SECURE CONNECTION TO THE MATRIX..." "${COLORS[BRIGHT_CYAN]}${COLORS[BOLD]}"
    digital_rain 3
    
    # Phase 3: System Information
    elite_system_info
    sleep 2
    
    # Phase 4: Neural Network Simulation
    echo ""
    hacker_typewriter "${ROBOT} ACTIVATING NEURAL NETWORK PROTOCOLS..." "${COLORS[BRIGHT_MAGENTA]}${COLORS[BOLD]}"
    neural_network_effect 2
    
    # Phase 5: System Breach Animation
    system_breach_animation
    
    # Phase 6: Quantum Tunnel Effect
    quantum_tunnel_effect
    
    # Phase 7: Plasma Wave Effect
    echo ""
    hacker_typewriter "${DIAMOND} INITIALIZING PLASMA CORE..." "${COLORS[BRIGHT_YELLOW]}${COLORS[BOLD]}"
    plasma_wave 2
    
    # Phase 8: Final Breach Sequence
    final_breach_sequence
    
    # Phase 9: Launch Setup Script
    clear_screen
    echo ""
    printf "${COLORS[BRIGHT_GREEN]}${COLORS[BOLD]}"
    cat << 'EOF'
    ╔══════════════════════════════════════════════════════════════════════════════════════════════════════════╗
    ║                              🎯 BREACH COMPLETE - ACCESSING TARGET SYSTEM 🎯                           ║
    ╚══════════════════════════════════════════════════════════════════════════════════════════════════════════╝
EOF
    printf "${COLORS[RESET]}\n"
    
    sleep 2
    
    # Check if rust_github_archiver directory exists
    if [ ! -d "rust_github_archiver" ]; then
        printf "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}${SKULL} ERROR: rust_github_archiver directory not found!${COLORS[RESET]}\n"
        printf "${COLORS[BRIGHT_YELLOW]}Please ensure you're running this from the GitArchiver root directory.${COLORS[RESET]}\n"
        exit 1
    fi
    
    # Launch the setup script
    echo ""
    hacker_typewriter "${FIRE} TRANSFERRING CONTROL TO ELITE SETUP INTERFACE..." "${COLORS[BRIGHT_RED]}${COLORS[BOLD]}"
    sleep 1
    
    clear_screen
    show_cursor
    
    # Execute the setup script
    cd rust_github_archiver && ./setup.sh
}

# ═══════════════════════════════════════════════════════════════════════════════
# 🚀 LAUNCH SEQUENCE INITIATED 🚀
# ═══════════════════════════════════════════════════════════════════════════════

main "$@"
