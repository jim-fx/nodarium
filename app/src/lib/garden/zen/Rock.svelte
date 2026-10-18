<script lang="ts">
  import { T } from '@threlte/core';
  import type { Texture } from 'three';
  import { createMoundGeometry, createRockGeometry } from '../rock-geometry';
  import type { Surface } from '../textures';
  import { COLORS, type RockPlacement } from './config';

  type Props = { rock: RockPlacement; stone: Surface; mossMap: Texture };
  const { rock, stone, mossMap }: Props = $props();

  const rockGeometry = $derived(createRockGeometry(rock.seed));
  const moundGeometry = $derived(createMoundGeometry(rock.seed + 3));
</script>

<T.Group position={[rock.x, 0, rock.z]} rotation.y={rock.rotation}>
  <T.Mesh
    geometry={rockGeometry}
    scale={[rock.radius, rock.height, rock.radius * 0.8]}
    position.y={-rock.height * 0.08}
    castShadow
    receiveShadow
  >
    <T.MeshStandardMaterial color={COLORS.stone} {...stone} />
  </T.Mesh>
  <T.Mesh
    geometry={moundGeometry}
    scale={[rock.radius * 1.3, 0.1 + rock.radius * 0.08, rock.radius * 1.15]}
    position={[rock.radius * 0.15, 0, rock.radius * 0.1]}
    castShadow
    receiveShadow
  >
    <T.MeshStandardMaterial color={COLORS.moss} map={mossMap} roughness={1} />
  </T.Mesh>
</T.Group>
