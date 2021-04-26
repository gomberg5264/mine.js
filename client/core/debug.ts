import { GUI } from 'dat.gui';
import Stats from 'stats.js';
import { BoxGeometry, DoubleSide, Mesh, MeshBasicMaterial, PlaneBufferGeometry } from 'three';
// import { AxesHelper, GridHelper } from 'three';

import { Helper } from '../utils';

import { Engine, Registry } from '.';

class Debug {
  public gui: dat.GUI;
  public stats: Stats;
  public dataWrapper: HTMLDivElement;
  public dataEntires: { ele: HTMLParagraphElement; obj: any; attribute: string; name: string }[] = [];
  public chunkHighlight: Mesh;
  public atlasTest: Mesh;

  constructor(public engine: Engine) {
    // dat.gui
    this.gui = new GUI();

    // FPS indicator
    this.stats = new Stats();

    const {
      world: { chunkSize, dimension, maxHeight },
    } = engine.config;
    const width = chunkSize * dimension;
    this.chunkHighlight = new Mesh(
      new BoxGeometry(width, maxHeight * dimension, width),
      new MeshBasicMaterial({ wireframe: true, side: DoubleSide }),
    );

    // move dat.gui panel to the top
    const { parentElement } = this.gui.domElement;
    if (parentElement) {
      Helper.applyStyle(parentElement, {
        zIndex: '1000000000',
      });
    }

    engine.on('ready', () => {
      this.makeDOM();
      this.setupAll();
      this.mount();

      engine.rendering.scene.add(this.chunkHighlight);
      this.chunkHighlight.visible = false;
    });

    engine.on('texture-loaded', () => {
      // textureTest
      const testBlock = new PlaneBufferGeometry(4, 4);
      const testMat = new MeshBasicMaterial({
        map: Registry.atlasUniform.value,
        side: DoubleSide,
        transparent: true,
        alphaTest: 0.5,
      });
      this.atlasTest = new Mesh(testBlock, testMat);
      this.atlasTest.position.set(0, 20, 0);
      this.atlasTest.visible = false;
      this.engine.rendering.scene.add(this.atlasTest);
    });
  }

  makeDataEntry = () => {
    const dataEntry = document.createElement('p');
    Helper.applyStyle(dataEntry, {
      margin: '0',
    });
    return dataEntry;
  };

  makeDOM = () => {
    this.dataWrapper = document.createElement('div');
    Helper.applyStyle(this.dataWrapper, {
      position: 'absolute',
      bottom: '0',
      left: '0',
      background: '#00000022',
      color: 'white',
      fontFamily: `'Trebuchet MS', sans-serif`,
      padding: '4px',
      display: 'flex',
      flexDirection: 'column-reverse',
      alignItems: 'flex-start',
      justifyContent: 'flex-start',
    });
  };

  mount = () => {
    const { domElement } = this.engine.container;
    domElement.appendChild(this.stats.dom);
    domElement.appendChild(this.dataWrapper);
  };

  setupAll = () => {
    // RENDERING
    const { rendering, registry, player, camera, world } = this.engine;
    const {
      options: { chunkSize, dimension },
    } = world;

    const renderingFolder = this.gui.addFolder('rendering');
    renderingFolder
      .add(rendering.sky.options, 'domeOffset', 200, 2000, 10)
      // @ts-ignore
      .onChange((value) => (rendering.sky.material.uniforms.offset.value = value));

    renderingFolder
      .addColor(rendering.sky.options, 'topColor')
      // @ts-ignore
      .onFinishChange((value) => rendering.sky.material.uniforms.topColor.value.set(value));
    renderingFolder
      .addColor(rendering.sky.options, 'bottomColor')
      // @ts-ignore
      .onFinishChange((value) => rendering.sky.material.uniforms.bottomColor.value.set(value));
    renderingFolder
      .addColor(rendering.options, 'clearColor')
      .onFinishChange((value) => rendering.renderer.setClearColor(value));

    renderingFolder.open();

    // WORLD
    const worldFolder = this.gui.addFolder('world');
    worldFolder.add(world.options, 'renderRadius', 1, 10, 1).onFinishChange((value) => {
      Registry.opaqueChunkMaterial.uniforms.uFogNear.value = value * 0.6 * chunkSize * dimension;
      Registry.opaqueChunkMaterial.uniforms.uFogFar.value = value * chunkSize * dimension;
    });
    this.registerDisplay('chunk', world, 'camChunkPosStr');

    // PLAYER
    const playerFolder = this.gui.addFolder('player');
    playerFolder.add(player.options, 'acceleration', 0, 5, 0.01);
    playerFolder.add(player.options, 'flyingInertia', 0, 5, 0.01);
    this.registerDisplay('looking at', player, 'lookBlockStr');

    // CAMERA
    // const cameraFolder = this.gui.addFolder('camera');
    this.registerDisplay('position', camera, 'voxelPositionStr');

    // REGISTRY
    const registryFolder = this.gui.addFolder('registry');
    registryFolder.add(
      {
        'toggle atlas': () => {
          this.atlasTest.visible = !this.atlasTest.visible;
        },
      },
      'toggle atlas',
    );

    // DEBUG
    const debugFolder = this.gui.addFolder('debug');
    debugFolder.add(
      {
        'toggle chunk highlight': () => {
          this.chunkHighlight.visible = !this.chunkHighlight.visible;
        },
      },
      'toggle chunk highlight',
    );
  };

  tick = () => {
    for (const { ele, name, attribute, obj } of this.dataEntires) {
      const newValue = obj[attribute];
      ele.innerHTML = `${name}: ${newValue}`;
    }

    const { camChunkPosStr } = this.engine.world;
    const [cx, cz] = Helper.parseChunkName(camChunkPosStr, ' ');
    const { chunkSize, maxHeight, dimension } = this.engine.world.options;
    this.chunkHighlight.position.set(
      (cx + 0.5) * chunkSize * dimension,
      0.5 * maxHeight * dimension,
      (cz + 0.5) * chunkSize * dimension,
    );
  };

  registerDisplay(name: string, object: any, attribute: string) {
    const wrapper = this.makeDataEntry();
    const newEntry = {
      ele: wrapper,
      obj: object,
      name,
      attribute,
    };
    this.dataEntires.push(newEntry);
    this.dataWrapper.appendChild(wrapper);
  }
}

export { Debug };
