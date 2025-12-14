/**
 * DecentPaste Landing Page Scripts
 * Platform detection, FAQ accordion, navbar, and scroll animations
 */

// =============================================================================
// Interactive P2P Network Graph
// =============================================================================

class NetworkGraph {
  constructor(canvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.nodes = [];
    this.edges = [];
    this.dataPackets = [];
    this.draggedNode = null;
    this.mousePos = { x: 0, y: 0 };
    this.hoveredNode = null;
    this.animationId = null;
    this.dpr = window.devicePixelRatio || 1;

    // Physics settings
    this.physics = {
      friction: 0.92,
      springStrength: 0.008,
      springLength: 180,
      repulsion: 8000,
      centerGravity: 0.0008,
      maxVelocity: 3,
    };

    // Device types with their icons and colors
    this.deviceTypes = [
      { type: 'macbook', label: 'MacBook Pro', color: '#a78bfa', icon: 'laptop' },
      { type: 'windows', label: 'Windows PC', color: '#60a5fa', icon: 'desktop' },
      { type: 'iphone', label: 'iPhone', color: '#f472b6', icon: 'phone' },
      { type: 'android', label: 'Android', color: '#4ade80', icon: 'phone' },
      { type: 'ipad', label: 'iPad', color: '#c084fc', icon: 'tablet' },
      { type: 'linux', label: 'Linux', color: '#fbbf24', icon: 'desktop' },
    ];

    this.init();
  }

  init() {
    this.resize();
    this.createNodes();
    this.createEdges();
    this.bindEvents();
    this.animate();
  }

  resize() {
    const rect = this.canvas.getBoundingClientRect();
    const oldWidth = this.width;
    this.width = rect.width;
    this.height = rect.height;
    this.canvas.width = this.width * this.dpr;
    this.canvas.height = this.height * this.dpr;
    this.ctx.scale(this.dpr, this.dpr);

    // Detect significant layout change (crossing mobile/tablet/desktop breakpoints)
    const wasMobile = oldWidth && oldWidth < 640;
    const isMobile = this.width < 640;
    const wasTablet = oldWidth && oldWidth >= 640 && oldWidth < 1024;
    const isTablet = this.width >= 640 && this.width < 1024;

    if (oldWidth && ((wasMobile !== isMobile) || (wasTablet !== isTablet))) {
      // Recreate graph for new breakpoint
      this.nodes = [];
      this.edges = [];
      this.dataPackets = [];
      this.createNodes();
      this.createEdges();
    } else if (this.nodes.length > 0) {
      // Just reposition existing nodes within bounds
      const margin = isMobile ? 40 : 60;
      this.nodes.forEach(node => {
        node.x = Math.max(margin, Math.min(this.width - margin, node.x));
        node.y = Math.max(margin, Math.min(this.height - margin, node.y));
        node.targetX = node.x;
        node.targetY = node.y;
      });
    }

    // Update physics for screen size
    this.physics.springLength = isMobile ? 100 : (isTablet ? 140 : 180);
    this.physics.repulsion = isMobile ? 4000 : (isTablet ? 6000 : 8000);
  }

  createNodes() {
    const centerX = this.width / 2;
    // Center vertically in the container (which is now constrained to top area)
    const centerY = this.height * 0.45;
    const isMobile = this.width < 640;
    const isTablet = this.width >= 640 && this.width < 1024;

    // Responsive settings
    const nodeRadius = isMobile ? 24 : (isTablet ? 28 : 32);
    const baseRadius = Math.min(this.width * 0.35, this.height * 0.35);
    const numNodes = isMobile ? 6 : (isTablet ? 7 : 8); // More nodes!

    // Create node pool with duplicates allowed
    const nodePool = [];
    while (nodePool.length < numNodes) {
      const shuffled = [...this.deviceTypes].sort(() => Math.random() - 0.5);
      nodePool.push(...shuffled);
    }
    const selectedDevices = nodePool.slice(0, numNodes);

    selectedDevices.forEach((device, i) => {
      // Distribute nodes in a ring, avoiding the center
      const angle = (i / numNodes) * Math.PI * 2 + Math.random() * 0.4 - 0.2;
      const r = baseRadius * (0.8 + Math.random() * 0.5);
      const x = centerX + Math.cos(angle) * r;
      const y = centerY + Math.sin(angle) * r;

      this.nodes.push({
        id: i,
        x: x,
        y: y,
        targetX: x,
        targetY: y,
        vx: 0,
        vy: 0,
        radius: nodeRadius,
        ...device,
        pulsePhase: Math.random() * Math.PI * 2,
        glowIntensity: 0,
        // Drift parameters for gentle floating
        driftPhaseX: Math.random() * Math.PI * 2,
        driftPhaseY: Math.random() * Math.PI * 2,
        driftSpeedX: 0.0003 + Math.random() * 0.0004,
        driftSpeedY: 0.0003 + Math.random() * 0.0004,
      });
    });
  }

  createEdges() {
    // Create a connected network - each node connects to 2-3 others
    const numNodes = this.nodes.length;
    const edgeSet = new Set();

    // First, create a spanning tree to ensure connectivity
    for (let i = 1; i < numNodes; i++) {
      const j = Math.floor(Math.random() * i);
      const key = `${Math.min(i, j)}-${Math.max(i, j)}`;
      if (!edgeSet.has(key)) {
        edgeSet.add(key);
        this.edges.push({
          source: this.nodes[i],
          target: this.nodes[j],
          pulseOffset: Math.random() * Math.PI * 2,
          active: Math.random() > 0.3,
        });
      }
    }

    // Add a few more random edges for visual interest
    const extraEdges = Math.floor(numNodes * 0.5);
    for (let e = 0; e < extraEdges; e++) {
      const i = Math.floor(Math.random() * numNodes);
      let j = Math.floor(Math.random() * numNodes);
      if (i === j) j = (j + 1) % numNodes;
      const key = `${Math.min(i, j)}-${Math.max(i, j)}`;
      if (!edgeSet.has(key)) {
        edgeSet.add(key);
        this.edges.push({
          source: this.nodes[i],
          target: this.nodes[j],
          pulseOffset: Math.random() * Math.PI * 2,
          active: Math.random() > 0.5,
        });
      }
    }
  }

  bindEvents() {
    // Mouse events
    this.canvas.addEventListener('mousedown', this.onMouseDown.bind(this));
    this.canvas.addEventListener('mousemove', this.onMouseMove.bind(this));
    this.canvas.addEventListener('mouseup', this.onMouseUp.bind(this));
    this.canvas.addEventListener('mouseleave', this.onMouseUp.bind(this));

    // Touch events
    this.canvas.addEventListener('touchstart', this.onTouchStart.bind(this), { passive: false });
    this.canvas.addEventListener('touchmove', this.onTouchMove.bind(this), { passive: false });
    this.canvas.addEventListener('touchend', this.onTouchEnd.bind(this));

    // Resize
    window.addEventListener('resize', () => {
      this.resize();
    });
  }

  getMousePos(e) {
    const rect = this.canvas.getBoundingClientRect();
    return {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    };
  }

  getTouchPos(e) {
    const rect = this.canvas.getBoundingClientRect();
    const touch = e.touches[0];
    return {
      x: touch.clientX - rect.left,
      y: touch.clientY - rect.top,
    };
  }

  findNodeAt(pos) {
    for (let i = this.nodes.length - 1; i >= 0; i--) {
      const node = this.nodes[i];
      const dx = pos.x - node.x;
      const dy = pos.y - node.y;
      if (dx * dx + dy * dy < (node.radius + 10) * (node.radius + 10)) {
        return node;
      }
    }
    return null;
  }

  onMouseDown(e) {
    const pos = this.getMousePos(e);
    this.draggedNode = this.findNodeAt(pos);
    if (this.draggedNode) {
      this.draggedNode.isDragging = true;
      this.canvas.style.cursor = 'grabbing';
    }
  }

  onMouseMove(e) {
    const pos = this.getMousePos(e);
    this.mousePos = pos;

    if (this.draggedNode) {
      this.draggedNode.targetX = pos.x;
      this.draggedNode.targetY = pos.y;
      this.draggedNode.x = pos.x;
      this.draggedNode.y = pos.y;
      this.draggedNode.vx = 0;
      this.draggedNode.vy = 0;
    } else {
      const hoveredNode = this.findNodeAt(pos);
      if (hoveredNode !== this.hoveredNode) {
        this.hoveredNode = hoveredNode;
        this.canvas.style.cursor = hoveredNode ? 'grab' : 'default';
      }
    }
  }

  onMouseUp() {
    if (this.draggedNode) {
      this.draggedNode.isDragging = false;
      this.draggedNode = null;
      this.canvas.style.cursor = this.hoveredNode ? 'grab' : 'default';
    }
  }

  onTouchStart(e) {
    e.preventDefault();
    const pos = this.getTouchPos(e);
    this.draggedNode = this.findNodeAt(pos);
    if (this.draggedNode) {
      this.draggedNode.isDragging = true;
    }
  }

  onTouchMove(e) {
    e.preventDefault();
    if (this.draggedNode && e.touches.length > 0) {
      const pos = this.getTouchPos(e);
      this.draggedNode.targetX = pos.x;
      this.draggedNode.targetY = pos.y;
      this.draggedNode.x = pos.x;
      this.draggedNode.y = pos.y;
      this.draggedNode.vx = 0;
      this.draggedNode.vy = 0;
    }
  }

  onTouchEnd() {
    if (this.draggedNode) {
      this.draggedNode.isDragging = false;
      this.draggedNode = null;
    }
  }

  updatePhysics() {
    const { friction, springStrength, springLength, repulsion, centerGravity, maxVelocity } = this.physics;
    const centerX = this.width / 2;
    const centerY = this.height * 0.45;
    const time = Date.now();

    // Apply forces between nodes
    for (let i = 0; i < this.nodes.length; i++) {
      const nodeA = this.nodes[i];
      if (nodeA.isDragging) continue;

      // Gentle drift force (slow floating)
      const driftX = Math.sin(time * nodeA.driftSpeedX + nodeA.driftPhaseX) * 0.015;
      const driftY = Math.sin(time * nodeA.driftSpeedY + nodeA.driftPhaseY) * 0.015;
      nodeA.vx += driftX;
      nodeA.vy += driftY;

      // Center gravity
      const dxCenter = centerX - nodeA.x;
      const dyCenter = centerY - nodeA.y;
      nodeA.vx += dxCenter * centerGravity;
      nodeA.vy += dyCenter * centerGravity;

      // Repulsion between nodes
      for (let j = i + 1; j < this.nodes.length; j++) {
        const nodeB = this.nodes[j];
        const dx = nodeB.x - nodeA.x;
        const dy = nodeB.y - nodeA.y;
        const distSq = dx * dx + dy * dy;
        const dist = Math.sqrt(distSq) || 1;
        const force = repulsion / distSq;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;

        if (!nodeA.isDragging) {
          nodeA.vx -= fx;
          nodeA.vy -= fy;
        }
        if (!nodeB.isDragging) {
          nodeB.vx += fx;
          nodeB.vy += fy;
        }
      }
    }

    // Spring forces along edges
    this.edges.forEach(edge => {
      const dx = edge.target.x - edge.source.x;
      const dy = edge.target.y - edge.source.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const diff = dist - springLength;
      const force = diff * springStrength;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;

      if (!edge.source.isDragging) {
        edge.source.vx += fx;
        edge.source.vy += fy;
      }
      if (!edge.target.isDragging) {
        edge.target.vx -= fx;
        edge.target.vy -= fy;
      }
    });

    // Update positions
    const margin = this.width < 640 ? 40 : 60;
    this.nodes.forEach(node => {
      if (node.isDragging) return;

      // Apply friction
      node.vx *= friction;
      node.vy *= friction;

      // Clamp velocity
      const speed = Math.sqrt(node.vx * node.vx + node.vy * node.vy);
      if (speed > maxVelocity) {
        node.vx = (node.vx / speed) * maxVelocity;
        node.vy = (node.vy / speed) * maxVelocity;
      }

      // Update position
      node.x += node.vx;
      node.y += node.vy;

      // Keep within bounds
      node.x = Math.max(margin, Math.min(this.width - margin, node.x));
      node.y = Math.max(margin, Math.min(this.height - margin, node.y));
    });
  }

  spawnDataPacket() {
    if (this.edges.length === 0) return;
    if (Math.random() > 0.02) return; // Spawn occasionally

    const edge = this.edges[Math.floor(Math.random() * this.edges.length)];
    if (!edge.active) return;

    const reverse = Math.random() > 0.5;
    this.dataPackets.push({
      edge: edge,
      progress: 0,
      speed: 0.008 + Math.random() * 0.008,
      reverse: reverse,
      size: 3 + Math.random() * 2,
    });
  }

  updateDataPackets() {
    this.dataPackets = this.dataPackets.filter(packet => {
      packet.progress += packet.speed;
      if (packet.progress >= 1) {
        // Trigger glow on receiving node
        const targetNode = packet.reverse ? packet.edge.source : packet.edge.target;
        targetNode.glowIntensity = 1;
        return false;
      }
      return true;
    });

    // Decay glow
    this.nodes.forEach(node => {
      node.glowIntensity *= 0.95;
    });
  }

  draw() {
    const ctx = this.ctx;
    const time = Date.now() * 0.001;

    // Clear canvas
    ctx.clearRect(0, 0, this.width, this.height);

    // Draw edges
    this.edges.forEach(edge => {
      const pulse = Math.sin(time * 2 + edge.pulseOffset) * 0.5 + 0.5;
      const opacity = edge.active ? 0.3 + pulse * 0.15 : 0.15;

      ctx.beginPath();
      ctx.moveTo(edge.source.x, edge.source.y);
      ctx.lineTo(edge.target.x, edge.target.y);
      ctx.strokeStyle = `rgba(255, 255, 255, ${opacity})`;
      ctx.lineWidth = edge.active ? 1.5 : 1;
      ctx.stroke();

      // Active edge glow
      if (edge.active) {
        ctx.beginPath();
        ctx.moveTo(edge.source.x, edge.source.y);
        ctx.lineTo(edge.target.x, edge.target.y);
        ctx.strokeStyle = `rgba(20, 184, 166, ${0.2 + pulse * 0.25})`;
        ctx.lineWidth = 3;
        ctx.stroke();
      }
    });

    // Draw data packets
    this.dataPackets.forEach(packet => {
      const edge = packet.edge;
      const t = packet.reverse ? 1 - packet.progress : packet.progress;
      const x = edge.source.x + (edge.target.x - edge.source.x) * t;
      const y = edge.source.y + (edge.target.y - edge.source.y) * t;

      // Glow
      const gradient = ctx.createRadialGradient(x, y, 0, x, y, packet.size * 5);
      gradient.addColorStop(0, 'rgba(20, 184, 166, 1)');
      gradient.addColorStop(0.4, 'rgba(20, 184, 166, 0.4)');
      gradient.addColorStop(1, 'rgba(20, 184, 166, 0)');
      ctx.beginPath();
      ctx.arc(x, y, packet.size * 5, 0, Math.PI * 2);
      ctx.fillStyle = gradient;
      ctx.fill();

      // Core
      ctx.beginPath();
      ctx.arc(x, y, packet.size, 0, Math.PI * 2);
      ctx.fillStyle = '#2dd4bf';
      ctx.fill();
    });

    // Draw nodes
    this.nodes.forEach(node => {
      const isHovered = node === this.hoveredNode || node === this.draggedNode;
      const pulse = Math.sin(time * 1.5 + node.pulsePhase) * 0.5 + 0.5;
      const scale = isHovered ? 1.15 : 1 + pulse * 0.03;
      const radius = node.radius * scale;

      // Outer glow (on data receive)
      if (node.glowIntensity > 0.01) {
        const gradient = ctx.createRadialGradient(node.x, node.y, radius, node.x, node.y, radius * 2.5);
        gradient.addColorStop(0, `rgba(20, 184, 166, ${node.glowIntensity * 0.6})`);
        gradient.addColorStop(1, 'rgba(20, 184, 166, 0)');
        ctx.beginPath();
        ctx.arc(node.x, node.y, radius * 2.5, 0, Math.PI * 2);
        ctx.fillStyle = gradient;
        ctx.fill();
      }

      // Node background with gradient
      const bgGradient = ctx.createRadialGradient(
        node.x - radius * 0.3,
        node.y - radius * 0.3,
        0,
        node.x,
        node.y,
        radius
      );
      bgGradient.addColorStop(0, 'rgba(45, 45, 50, 0.98)');
      bgGradient.addColorStop(1, 'rgba(30, 30, 35, 0.98)');

      ctx.beginPath();
      ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
      ctx.fillStyle = bgGradient;
      ctx.fill();

      // Border
      ctx.beginPath();
      ctx.arc(node.x, node.y, radius, 0, Math.PI * 2);
      ctx.strokeStyle = isHovered
        ? node.color
        : `rgba(255, 255, 255, ${0.15 + pulse * 0.1})`;
      ctx.lineWidth = isHovered ? 2.5 : 1.5;
      ctx.stroke();

      // Colored accent ring
      ctx.beginPath();
      ctx.arc(node.x, node.y, radius - 3, 0, Math.PI * 2);
      ctx.strokeStyle = isHovered ? node.color : `${node.color}88`;
      ctx.lineWidth = 2;
      ctx.stroke();

      // Draw icon
      this.drawIcon(ctx, node.x, node.y - 4, node.icon, node.color, isHovered);

      // Draw label (responsive font size)
      const fontSize = this.width < 640 ? 9 : 11;
      ctx.font = `${fontSize}px Inter, system-ui, sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'top';
      ctx.fillStyle = isHovered ? 'rgba(255, 255, 255, 1)' : 'rgba(255, 255, 255, 0.8)';
      ctx.fillText(node.label, node.x, node.y + radius + 6);
    });
  }

  drawIcon(ctx, x, y, iconType, color, isHovered) {
    const size = 16;
    ctx.strokeStyle = isHovered ? color : 'rgba(255, 255, 255, 0.7)';
    ctx.lineWidth = 1.5;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';

    switch (iconType) {
      case 'laptop':
        // Laptop screen
        ctx.beginPath();
        ctx.roundRect(x - size/2, y - size/2.5, size, size * 0.6, 2);
        ctx.stroke();
        // Laptop base
        ctx.beginPath();
        ctx.moveTo(x - size/1.6, y + size/4);
        ctx.lineTo(x + size/1.6, y + size/4);
        ctx.stroke();
        break;

      case 'desktop':
        // Monitor
        ctx.beginPath();
        ctx.roundRect(x - size/2, y - size/2, size, size * 0.7, 2);
        ctx.stroke();
        // Stand
        ctx.beginPath();
        ctx.moveTo(x, y + size * 0.2);
        ctx.lineTo(x, y + size * 0.4);
        ctx.moveTo(x - size/4, y + size * 0.4);
        ctx.lineTo(x + size/4, y + size * 0.4);
        ctx.stroke();
        break;

      case 'phone':
        // Phone body
        ctx.beginPath();
        ctx.roundRect(x - size/4, y - size/2, size/2, size, 3);
        ctx.stroke();
        // Home button / notch
        ctx.beginPath();
        ctx.arc(x, y + size/3, 2, 0, Math.PI * 2);
        ctx.stroke();
        break;

      case 'tablet':
        // Tablet body
        ctx.beginPath();
        ctx.roundRect(x - size/2.5, y - size/2, size * 0.8, size, 3);
        ctx.stroke();
        // Home button
        ctx.beginPath();
        ctx.arc(x, y + size/3, 2, 0, Math.PI * 2);
        ctx.stroke();
        break;
    }
  }

  animate() {
    this.updatePhysics();
    this.spawnDataPacket();
    this.updateDataPackets();
    this.draw();
    this.animationId = requestAnimationFrame(() => this.animate());
  }

  destroy() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
    }
  }
}

// Initialize network graph
function initNetworkGraph() {
  const canvas = document.getElementById('network-graph');
  if (!canvas) return;

  new NetworkGraph(canvas);
}

// =============================================================================
// Platform Detection
// =============================================================================

const platformConfig = {
  windows: {
    name: 'Windows',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.9-1.801"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste_x64-setup.exe',
  },
  macos: {
    name: 'macOS',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste_x64.dmg',
  },
  linux: {
    name: 'Linux',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2.025.134.063.198.114.333l.003.003c.391.778 1.113 1.132 1.884 1.071.771-.06 1.592-.536 2.257-1.306.631-.765 1.683-1.084 2.378-1.503.348-.199.629-.469.649-.853.023-.4-.2-.811-.714-1.376v-.097l-.003-.003c-.17-.2-.25-.535-.338-.926-.085-.401-.182-.786-.492-1.046h-.003c-.059-.054-.123-.067-.188-.135a.357.357 0 00-.19-.064c.431-1.278.264-2.55-.173-3.694-.533-1.41-1.465-2.638-2.175-3.483-.796-1.005-1.576-1.957-1.56-3.368.026-2.152.236-6.133-3.544-6.139z"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste_amd64.AppImage',
  },
  android: {
    name: 'Android',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M17.523 15.3414c-.5511 0-.9993-.4486-.9993-.9997s.4483-.9993.9993-.9993c.5511 0 .9993.4483.9993.9993.0001.5511-.4482.9997-.9993.9997m-11.046 0c-.5511 0-.9993-.4486-.9993-.9997s.4482-.9993.9993-.9993c.5511 0 .9993.4483.9993.9993 0 .5511-.4483.9997-.9993.9997m11.4045-6.02l1.9973-3.4592a.416.416 0 00-.1521-.5676.416.416 0 00-.5676.1521l-2.0223 3.503C15.5902 8.2439 13.8533 7.8508 12 7.8508s-3.5902.3931-5.1367 1.0989L4.841 5.4467a.4161.4161 0 00-.5677-.1521.4157.4157 0 00-.1521.5676l1.9973 3.4592C2.6889 11.1867.3432 14.6589 0 18.761h24c-.3435-4.1021-2.6892-7.5743-6.1185-9.4396"/></svg>`,
    downloadUrl: 'https://github.com/decentpaste/decentpaste/releases/latest/download/DecentPaste.apk',
  },
  ios: {
    name: 'iOS',
    icon: `<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>`,
    downloadUrl: '#downloads',
  },
  unknown: {
    name: 'Your Platform',
    icon: `<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/></svg>`,
    downloadUrl: '#downloads',
  },
};

/**
 * Detect the user's operating system
 */
function detectPlatform() {
  const userAgent = navigator.userAgent.toLowerCase();
  const platform = navigator.platform?.toLowerCase() || '';

  // iOS detection (must come before macOS - iPad can report as Mac)
  if (
    /ipad|iphone|ipod/.test(userAgent) ||
    (platform === 'macintel' && navigator.maxTouchPoints > 1)
  ) {
    return 'ios';
  }

  // Android detection
  if (/android/.test(userAgent)) {
    return 'android';
  }

  // Windows detection
  if (/win/.test(platform) || /windows/.test(userAgent)) {
    return 'windows';
  }

  // macOS detection
  if (/mac/.test(platform) && !/iphone|ipad|ipod/.test(userAgent)) {
    return 'macos';
  }

  // Linux detection
  if (/linux/.test(platform) && !/android/.test(userAgent)) {
    return 'linux';
  }

  return 'unknown';
}

/**
 * Update the hero download button based on detected platform
 */
function updateHeroButton() {
  const platform = detectPlatform();
  const config = platformConfig[platform];

  const btn = document.getElementById('primary-download');
  const iconEl = document.getElementById('platform-icon');
  const nameEl = document.getElementById('platform-name');

  if (btn && config && iconEl && nameEl) {
    iconEl.innerHTML = config.icon;
    nameEl.textContent = config.name;
    btn.href = config.downloadUrl;

    // Highlight the matching download card
    const downloadCard = document.getElementById(`download-${platform}`);
    if (downloadCard) {
      downloadCard.classList.add('highlighted');
    }
  }
}

// =============================================================================
// FAQ Accordion
// =============================================================================

/**
 * Toggle FAQ item open/closed
 */
function toggleFaq(button) {
  const item = button.closest('.faq-item');
  const isActive = item.classList.contains('active');

  // Close all other FAQ items
  document.querySelectorAll('.faq-item.active').forEach((faqItem) => {
    if (faqItem !== item) {
      faqItem.classList.remove('active');
    }
  });

  // Toggle current item
  item.classList.toggle('active', !isActive);
}

// Make toggleFaq available globally
window.toggleFaq = toggleFaq;

// =============================================================================
// Mobile Menu
// =============================================================================

/**
 * Initialize mobile menu functionality
 */
function initMobileMenu() {
  const menuBtn = document.getElementById('mobile-menu-btn');
  const mobileMenu = document.getElementById('mobile-menu');
  const menuIconOpen = document.getElementById('menu-icon-open');
  const menuIconClose = document.getElementById('menu-icon-close');

  if (!menuBtn || !mobileMenu) return;

  menuBtn.addEventListener('click', () => {
    const isOpen = !mobileMenu.classList.contains('hidden');

    if (isOpen) {
      mobileMenu.classList.add('hidden');
      menuIconOpen?.classList.remove('hidden');
      menuIconClose?.classList.add('hidden');
    } else {
      mobileMenu.classList.remove('hidden');
      menuIconOpen?.classList.add('hidden');
      menuIconClose?.classList.remove('hidden');
    }
  });

  // Close menu when clicking a link
  mobileMenu.querySelectorAll('a').forEach((link) => {
    link.addEventListener('click', () => {
      mobileMenu.classList.add('hidden');
      menuIconOpen?.classList.remove('hidden');
      menuIconClose?.classList.add('hidden');
    });
  });
}

// =============================================================================
// Navbar Background
// =============================================================================

/**
 * Update navbar background on scroll
 */
function initNavbarScroll() {
  const navbar = document.getElementById('navbar');
  if (!navbar) return;

  const updateNavbar = () => {
    if (window.scrollY > 50) {
      navbar.classList.add('scrolled');
    } else {
      navbar.classList.remove('scrolled');
    }
  };

  window.addEventListener('scroll', updateNavbar, { passive: true });
  updateNavbar();
}

// =============================================================================
// Scroll Animations
// =============================================================================

/**
 * Initialize fade-in animations on scroll
 */
function initScrollAnimations() {
  const observerOptions = {
    threshold: 0.1,
    rootMargin: '0px 0px -50px 0px',
  };

  const observer = new IntersectionObserver((entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add('visible');
        observer.unobserve(entry.target);
      }
    });
  }, observerOptions);

  document.querySelectorAll('.fade-in-up').forEach((el) => {
    observer.observe(el);
  });
}

// =============================================================================
// Smooth Scroll for Anchor Links
// =============================================================================

function initSmoothScroll() {
  document.querySelectorAll('a[href^="#"]').forEach((anchor) => {
    anchor.addEventListener('click', function (e) {
      const href = this.getAttribute('href');
      if (href === '#') return;

      const target = document.querySelector(href);
      if (target) {
        e.preventDefault();
        const offsetTop = target.getBoundingClientRect().top + window.pageYOffset - 80;
        window.scrollTo({
          top: offsetTop,
          behavior: 'smooth',
        });
      }
    });
  });
}

// =============================================================================
// Initialization
// =============================================================================

document.addEventListener('DOMContentLoaded', () => {
  initNetworkGraph();
  updateHeroButton();
  initMobileMenu();
  initNavbarScroll();
  initScrollAnimations();
  initSmoothScroll();
});
