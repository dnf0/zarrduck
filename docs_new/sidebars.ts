import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    {
      type: 'category',
      label: 'Using Eider',
      items: [
        'usage/installation',
        'usage/cli_tui',
        'usage/sql_reference',
        'usage/exporting',
      ],
    },
    {
      type: 'category',
      label: 'Engineering Deep-Dive',
      items: [
        'engineering/architecture',
        'engineering/cog_virtualization',
        'engineering/spatial_pruning',
        'engineering/benchmarks',
      ],
    },
  ],
};

export default sidebars;
