import { GUI } from 'dat.gui';
import Stats from 'stats.js';
import { BoxGeometry, DoubleSide, Mesh, MeshBasicMaterial } from 'three';
// import { AxesHelper, GridHelper } from 'three';

import { Engine } from '..';
import { Helper } from '../utils';

class Debug {
  public engine: Engine;
  public gui: dat.GUI;
  public stats: Stats;
  public dataWrapper: HTMLDivElement;
  public dataEntires: { ele: HTMLParagraphElement; obj: any; attribute: string; name: string }[] = [];
  public chunkHighlight: Mesh;

  constructor(engine: Engine) {
    this.engine = engine;

    // dat.gui
    this.gui = new GUI();

    // FPS indicator
    this.stats = new Stats();

    const {
      worldOptions: { chunkSize, dimension, maxHeight },
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

      // const {
      //   rendering: { scene },
      //   world: {
      //     options: { chunkSize, dimension },
      //   },
      // } = this.engine;

      // const axesHelper = new AxesHelper(5);
      // engine.rendering.scene.add(axesHelper);

      // const gridHelper = new GridHelper(2 * chunkSize * dimension, 2 * chunkSize);
      // scene.add(gridHelper);
    });

    engine.on('start', () => {
      // textureTest
      // const testBlock = new PlaneBufferGeometry(4, 4);
      // const testMat = new MeshBasicMaterial({ map: this.engine.registry.mergedTexture, side: DoubleSide });
      // const testMesh = new Mesh(testBlock, testMat);
      // testMesh.position.set(0, 20, 0);
      // this.engine.rendering.scene.add(testMesh);
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
    const { rendering, camera, world } = this.engine;

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
    worldFolder.add(world.options, 'renderRadius', 1, 10, 1);
    this.registerDisplay('chunk', world, 'camChunkPosStr');

    // CAMERA
    const cameraFolder = this.gui.addFolder('camera');
    cameraFolder.add(camera.options, 'acceleration', 0, 5, 0.01);
    cameraFolder.add(camera.options, 'flyingInertia', 0, 5, 0.01);
    this.registerDisplay('position', camera, 'voxelPositionStr');
    this.registerDisplay('looking at', camera, 'lookBlockStr');
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
