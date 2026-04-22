import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';

export const PIECE_MODEL_ATTRIBUTION =
	'Game piece model by 3P3D (Thingiverse #84406), CC BY';

export const DEFAULT_PIECE_URL = '/models/Game_Piece_mm.glb';

const PIECE_TARGET_HEIGHT = 0.13;

let cachedTemplate: Promise<THREE.BufferGeometry> | null = null;

/**
 * Load the pawn GLB and normalize it: the returned geometry is centered on
 * its footprint (origin at the base), scaled so its height ≈ 0.13 world
 * units. The geometry is shared across every clone; only materials are
 * per-pawn.
 */
export function loadPieceGeometry(url = DEFAULT_PIECE_URL): Promise<THREE.BufferGeometry> {
	if (cachedTemplate) return cachedTemplate;
	cachedTemplate = new Promise((resolve, reject) => {
		const loader = new GLTFLoader();
		loader.load(
			url,
			(gltf) => {
				let found: THREE.Mesh | null = null;
				gltf.scene.traverse((obj) => {
					if (!found && (obj as THREE.Mesh).isMesh) {
						found = obj as THREE.Mesh;
					}
				});
				if (!found) {
					reject(new Error(`No mesh found in GLB at ${url}`));
					return;
				}
				const geom = (found as THREE.Mesh).geometry.clone();
				geom.computeBoundingBox();
				const bb = geom.boundingBox!;
				const height = bb.max.y - bb.min.y;
				const scale = PIECE_TARGET_HEIGHT / height;
				// Center in x/z, set base at y = 0.
				geom.translate(
					-(bb.min.x + bb.max.x) / 2,
					-bb.min.y,
					-(bb.min.z + bb.max.z) / 2
				);
				geom.scale(scale, scale, scale);
				geom.computeVertexNormals();
				resolve(geom);
			},
			undefined,
			(err) => reject(err instanceof Error ? err : new Error(String(err)))
		);
	});
	return cachedTemplate;
}

/**
 * Build a pawn mesh for a given player color. Each call creates a fresh
 * `MeshStandardMaterial` so per-pawn tinting / emissive selection is
 * independent. Geometry is shared via the cached template.
 */
export function makePiece(
	geometry: THREE.BufferGeometry,
	color: THREE.ColorRepresentation
): THREE.Mesh {
	const mat = new THREE.MeshStandardMaterial({
		color: new THREE.Color(color),
		roughness: 0.55,
		metalness: 0.1
	});
	const mesh = new THREE.Mesh(geometry, mat);
	mesh.castShadow = true;
	mesh.receiveShadow = false;
	return mesh;
}
