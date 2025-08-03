import React, { useRef, useMemo } from 'react';
import { Canvas, useFrame } from '@react-three/fiber';
import * as THREE from 'three';

interface LavaLampBackgroundProps {
  health: 'healthy' | 'warning' | 'critical';
  isScanning: boolean;
}

interface LavaBlobProps {
  position: [number, number, number];
  scale: number;
  color: string;
  speed: number;
}

function LavaBlob({ position, scale, color, speed }: LavaBlobProps) {
  const meshRef = useRef<THREE.Mesh>(null);
  const startTime = useRef(Date.now());

  useFrame((state) => {
    if (meshRef.current) {
      const time = (Date.now() - startTime.current) * 0.001 * speed;
      
      // Organic floating movement
      meshRef.current.position.x = position[0] + Math.sin(time * 0.5) * 2;
      meshRef.current.position.y = position[1] + Math.cos(time * 0.3) * 1.5;
      meshRef.current.position.z = position[2] + Math.sin(time * 0.7) * 1;
      
      // Organic scaling
      const baseScale = scale;
      const scaleVariation = Math.sin(time * 0.8) * 0.2;
      meshRef.current.scale.setScalar(baseScale + scaleVariation);
      
      // Rotation
      meshRef.current.rotation.x = time * 0.2;
      meshRef.current.rotation.y = time * 0.3;
    }
  });

  return (
    <mesh ref={meshRef} position={position}>
      <sphereGeometry args={[1, 32, 32]} />
      <meshStandardMaterial
        color={color}
        transparent
        opacity={0.6}
        roughness={0.1}
        metalness={0.3}
        emissive={color}
        emissiveIntensity={0.2}
      />
    </mesh>
  );
}

function LavaLampScene({ health, isScanning }: LavaLampBackgroundProps) {
  const blobs = useMemo(() => {
    const healthColors = {
      healthy: ['#00FF41', '#00CC33', '#00AA22'],
      warning: ['#FFFF00', '#FFCC00', '#FF9900'],
      critical: ['#FF0041', '#CC0033', '#AA0022']
    };

    const colors = healthColors[health];
    const blobCount = isScanning ? 12 : 8;
    
    return Array.from({ length: blobCount }, (_, i) => ({
      id: i,
      position: [
        (Math.random() - 0.5) * 20,
        (Math.random() - 0.5) * 15,
        (Math.random() - 0.5) * 10
      ] as [number, number, number],
      scale: 0.5 + Math.random() * 1.5,
      color: colors[i % colors.length],
      speed: isScanning ? 0.8 + Math.random() * 0.4 : 0.3 + Math.random() * 0.2
    }));
  }, [health, isScanning]);

  return (
    <>
      {/* Ambient lighting */}
      <ambientLight intensity={0.3} />
      
      {/* Dynamic lighting based on health */}
      <pointLight
        position={[10, 10, 10]}
        intensity={1}
        color={health === 'healthy' ? '#00FF41' : health === 'warning' ? '#FFFF00' : '#FF0041'}
      />
      
      {/* Lava blobs */}
      {blobs.map(blob => (
        <LavaBlob
          key={blob.id}
          position={blob.position}
          scale={blob.scale}
          color={blob.color}
          speed={blob.speed}
        />
      ))}
      
      {/* Scanning effect - additional particles */}
      {isScanning && (
        <>
          {Array.from({ length: 20 }, (_, i) => (
            <mesh
              key={`particle-${i}`}
              position={[
                (Math.random() - 0.5) * 30,
                (Math.random() - 0.5) * 20,
                (Math.random() - 0.5) * 15
              ]}
            >
              <sphereGeometry args={[0.1, 8, 8]} />
              <meshBasicMaterial
                color={health === 'healthy' ? '#00FF41' : health === 'warning' ? '#FFFF00' : '#FF0041'}
                transparent
                opacity={0.8}
              />
            </mesh>
          ))}
        </>
      )}
    </>
  );
}

export function LavaLampBackground({ health, isScanning }: LavaLampBackgroundProps) {
  return (
    <div className="lava-lamp-container">
      <Canvas
        camera={{ position: [0, 0, 15], fov: 60 }}
        style={{ background: 'transparent' }}
      >
        <LavaLampScene health={health} isScanning={isScanning} />
      </Canvas>
      
      {/* CSS-based lava bubbles for additional effect */}
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        {Array.from({ length: 6 }, (_, i) => (
          <div
            key={i}
            className={`lava-bubble w-32 h-32 ${
              health === 'healthy' ? 'text-lava-green' :
              health === 'warning' ? 'text-lava-yellow' : 'text-lava-red'
            }`}
            style={{
              left: `${Math.random() * 100}%`,
              top: `${Math.random() * 100}%`,
              animationDelay: `${Math.random() * 8}s`,
              animationDuration: `${8 + Math.random() * 4}s`
            }}
          />
        ))}
      </div>
    </div>
  );
}
