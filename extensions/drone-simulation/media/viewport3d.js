// Embedded 3D viewport for the Vyuta simulation panel (Phase 3).
//
// A small Three.js scene — ground grid, home marker, a quadcopter built from
// primitives, and a flight trail — driven by pose frames from the sim-manager
// sidecar. Includes a lightweight orbit camera (drag to orbit, scroll to zoom)
// so no extra controls dependency is vendored.
//
// Frame mapping: the sidecar streams local ENU metres (x=east, y=north, z=up).
// Three.js is Y-up, so we map (x, y, z)_ENU -> (x, z, -y)_three.

import * as THREE from "three";

export class Viewport3D {
  constructor(canvas) {
    this.canvas = canvas;
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0b0f17);
    this.scene.fog = new THREE.Fog(0x0b0f17, 70, 220);

    this.camera = new THREE.PerspectiveCamera(55, 1, 0.1, 2000);

    // Orbit-camera state (spherical around `target`).
    this.camAz = Math.PI * 0.25;
    this.camEl = 0.55;
    this.camDist = 28;
    this.target = new THREE.Vector3(0, 2, 0);

    this.latest = null;
    this.armed = false;

    this._initLights();
    this._initGround();
    this._initDrone();
    this._initTrail();
    this._bindPointer();

    this._resize();
    this._onResize = () => this._resize();
    window.addEventListener("resize", this._onResize);

    this._tick = this._tick.bind(this);
    requestAnimationFrame(this._tick);
  }

  _initLights() {
    this.scene.add(new THREE.HemisphereLight(0xbfd4ff, 0x202830, 1.15));
    const dir = new THREE.DirectionalLight(0xffffff, 1.4);
    dir.position.set(20, 40, 10);
    this.scene.add(dir);
  }

  _initGround() {
    const grid = new THREE.GridHelper(200, 80, 0x3a4a63, 0x223047);
    grid.material.transparent = true;
    grid.material.opacity = 0.6;
    this.scene.add(grid);

    const plane = new THREE.Mesh(
      new THREE.PlaneGeometry(200, 200),
      new THREE.MeshStandardMaterial({ color: 0x141b26, roughness: 1, metalness: 0 })
    );
    plane.rotation.x = -Math.PI / 2;
    plane.position.y = -0.02;
    this.scene.add(plane);

    // Home pad ring.
    const ring = new THREE.Mesh(
      new THREE.RingGeometry(0.6, 0.85, 40),
      new THREE.MeshBasicMaterial({ color: 0x49d0a0, side: THREE.DoubleSide })
    );
    ring.rotation.x = -Math.PI / 2;
    ring.position.y = 0.01;
    this.scene.add(ring);

    this.scene.add(new THREE.AxesHelper(3));
  }

  _initDrone() {
    const drone = new THREE.Group();
    const armMat = new THREE.MeshStandardMaterial({ color: 0x2b3442, roughness: 0.6, metalness: 0.3 });
    const bodyMat = new THREE.MeshStandardMaterial({ color: 0x4f9dff, roughness: 0.4, metalness: 0.4 });
    const rotorMat = new THREE.MeshStandardMaterial({
      color: 0x9fb4cc,
      roughness: 0.5,
      transparent: true,
      opacity: 0.85,
    });
    const noseMat = new THREE.MeshStandardMaterial({ color: 0xff5a5a, roughness: 0.5 });

    drone.add(new THREE.Mesh(new THREE.BoxGeometry(1.1, 0.35, 1.1), bodyMat));

    // Nose marker points +X (forward / east at yaw 0).
    const nose = new THREE.Mesh(new THREE.BoxGeometry(0.5, 0.18, 0.22), noseMat);
    nose.position.set(0.7, 0.05, 0);
    drone.add(nose);

    const armLen = 1.5;
    this.rotors = [];
    for (let i = 0; i < 4; i++) {
      const ang = Math.PI / 4 + (i * Math.PI) / 2; // X configuration
      const dx = Math.cos(ang) * armLen;
      const dz = Math.sin(ang) * armLen;

      const arm = new THREE.Mesh(new THREE.BoxGeometry(armLen, 0.12, 0.12), armMat);
      arm.position.set(dx / 2, 0, dz / 2);
      arm.rotation.y = -ang;
      drone.add(arm);

      const motor = new THREE.Mesh(new THREE.CylinderGeometry(0.18, 0.18, 0.22, 12), armMat);
      motor.position.set(dx, 0.12, dz);
      drone.add(motor);

      const rotor = new THREE.Mesh(new THREE.CylinderGeometry(0.6, 0.6, 0.04, 24), rotorMat);
      rotor.position.set(dx, 0.25, dz);
      drone.add(rotor);
      this.rotors.push(rotor);
    }

    drone.rotation.order = "YXZ";
    this.drone = drone;
    this.scene.add(drone);
  }

  _initTrail() {
    this.trailMax = 600;
    this.trailCount = 0;
    this.trailLast = null;
    this.trailPos = new Float32Array(this.trailMax * 3);
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(this.trailPos, 3));
    geo.setDrawRange(0, 0);
    this.trailGeo = geo;
    this.trail = new THREE.Line(
      geo,
      new THREE.LineBasicMaterial({ color: 0x49d0a0, transparent: true, opacity: 0.7 })
    );
    this.scene.add(this.trail);
  }

  _bindPointer() {
    let dragging = false;
    let lx = 0;
    let ly = 0;
    this.canvas.addEventListener("pointerdown", (e) => {
      dragging = true;
      lx = e.clientX;
      ly = e.clientY;
      this.canvas.setPointerCapture(e.pointerId);
    });
    this.canvas.addEventListener("pointerup", (e) => {
      dragging = false;
      try {
        this.canvas.releasePointerCapture(e.pointerId);
      } catch (_e) {
        /* ignore */
      }
    });
    this.canvas.addEventListener("pointermove", (e) => {
      if (!dragging) return;
      this.camAz -= (e.clientX - lx) * 0.005;
      this.camEl += (e.clientY - ly) * 0.005;
      this.camEl = Math.max(0.05, Math.min(1.5, this.camEl));
      lx = e.clientX;
      ly = e.clientY;
    });
    this.canvas.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        this.camDist *= 1 + e.deltaY * 0.001;
        this.camDist = Math.max(6, Math.min(140, this.camDist));
      },
      { passive: false }
    );
  }

  /** ENU pose -> three.js position vector. */
  static toThree(x, y, z) {
    return new THREE.Vector3(x, z, -y);
  }

  /** Feed a pose frame from the sidecar. */
  update(pose) {
    this.latest = pose;
    this.armed = !!pose.armed;
  }

  /** Clear the flight trail and recentre the camera target. */
  reset() {
    this.trailCount = 0;
    this.trailLast = null;
    this.trailGeo.setDrawRange(0, 0);
    this.target.set(0, 2, 0);
  }

  _pushTrail(p) {
    if (this.trailLast && p.distanceTo(this.trailLast) < 0.3) return;
    if (this.trailCount >= this.trailMax) {
      // Shift left by one point to keep a rolling window.
      this.trailPos.copyWithin(0, 3);
      this.trailCount = this.trailMax - 1;
    }
    const o = this.trailCount * 3;
    this.trailPos[o] = p.x;
    this.trailPos[o + 1] = p.y;
    this.trailPos[o + 2] = p.z;
    this.trailCount++;
    this.trailGeo.setDrawRange(0, this.trailCount);
    this.trailGeo.attributes.position.needsUpdate = true;
    this.trailLast = p.clone();
  }

  _tick() {
    requestAnimationFrame(this._tick);
    this._resize();

    if (this.latest) {
      const p = this.latest;
      const pos = Viewport3D.toThree(p.x, p.y, p.z);
      this.drone.position.copy(pos);
      // ENU yaw about +Z maps directly to three rotation about +Y.
      this.drone.rotation.set(p.pitch || 0, p.yaw || 0, p.roll || 0, "YXZ");
      this._pushTrail(pos);
      // Smoothly follow the vehicle.
      this.target.lerp(new THREE.Vector3(pos.x, pos.y + 1.0, pos.z), 0.08);
    }

    // Spin rotors (faster when armed).
    const spin = this.armed ? 0.9 : 0.05;
    for (const r of this.rotors) r.rotation.y += spin;

    // Position the orbit camera.
    const ce = Math.cos(this.camEl);
    this.camera.position.set(
      this.target.x + this.camDist * ce * Math.cos(this.camAz),
      this.target.y + this.camDist * Math.sin(this.camEl),
      this.target.z + this.camDist * ce * Math.sin(this.camAz)
    );
    this.camera.lookAt(this.target);

    this.renderer.render(this.scene, this.camera);
  }

  _resize() {
    const w = this.canvas.clientWidth;
    const h = this.canvas.clientHeight;
    if (w === 0 || h === 0) return;
    if (this.canvas.width === w && this.canvas.height === h) return;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }
}
