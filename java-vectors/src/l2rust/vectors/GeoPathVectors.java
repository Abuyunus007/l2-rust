package l2rust.vectors;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;

import org.l2jmobius.gameserver.config.GeoEngineConfig;
import org.l2jmobius.gameserver.geoengine.GeoEngine;
import org.l2jmobius.gameserver.geoengine.geodata.IBlock;
import org.l2jmobius.gameserver.geoengine.geodata.IRegion;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.FlatBlock;
import org.l2jmobius.gameserver.geoengine.geodata.regions.Region;
import org.l2jmobius.gameserver.geoengine.pathfinding.GeoLocation;
import org.l2jmobius.gameserver.geoengine.pathfinding.PathFinding;

/**
 * Vectors for the pathfinding port (M3-GEO-02). Every expected path comes from the real L2JMobius {@link PathFinding#findPath} (which runs the real {@code NodeBuffer} and {@link GeoLocation}).
 * <p>
 * How it is driven:
 * <ul>
 * <li>The real {@code GeoEngine} singleton loads the whole world of the first set from {@code GeoEngineConfig.GEODATA_PATH} on its first {@code getInstance()}. The second set is loaded by calling the protected constructor by reflection again: the region table is {@code static}, so the singleton sees the new data (same trick as {@link GeoEngineVectors}).</li>
 * <li>{@code PathFinding} reads its config in the constructor ({@code NodeBuffer} reads the weights when a buffer is allocated, once per thread), so every config variant gets a fresh {@code PathFinding} created by reflection. The {@code GeoEngineConfig} statics are set directly ({@code GeoEngineConfig.load()} and its ini file are not used).</li>
 * <li>Doors and fences: none (real {@code DoorData} / {@code FenceData} with no data files, like {@link GeoEngineVectors}); the run aborts if the working directory has those files. Only instance id 0.</li>
 * </ul>
 * Rules: no clock, no unseeded randomness; all sample points come from {@code new Random(seed)} and are written into the JSON.
 * <p>
 * Case format: {@code "x y z tx ty tz instance playable > <result>"}, world coordinates, playable 0/1. The result is {@code null} (Java returned {@code null}) or {@code "<count> nodeX,nodeY,geoHeight,nswe ..."} with the waypoints in order ({@code getNodeX()}, {@code getNodeY()}, {@code getZ()} and the packed NSWE bitmask); {@code getX()} / {@code getY()} are checked here to be the world conversion of the node.
 */
public final class GeoPathVectors
{
	/** Regions of {@code docs/analysis/05-geodata.md}, as {@code {regionX, regionY}}. */
	private static final int[][] REGIONS =
	{
		{12, 24},
		{13, 21},
		{14, 23},
		{16, 10},
		{22, 22},
		{17, 10}
	};

	/** How many tile positions without a geodata file are probed (null region = no geo). */
	private static final int MISSING_REGIONS = 2;

	private static final long SEED = 20260607L;

	/** Cases per region for the first (shipped) variant and for each other variant. */
	private static final int BASE_CASES = 70;
	private static final int VARIANT_CASES = 24;

	private static final String BUFFERS = "100x6;128x6;192x6;256x4;320x4;384x4;500x2";

	/** One config variant: name, AdvancedDiagonalStrategy, Low/Medium/High/Diagonal weight (strings, parsed with Float.parseFloat), MaxPostfilterPasses. */
	private static final Object[][] VARIANTS =
	{
		// The shipped GeoEngine.ini.
		{"shipped", false, "0.5", "2", "3", "0.707", 3},
		// The defaults in GeoEngineConfig.load() when the key is missing.
		{"advanced_diagonal", true, "0.5", "2", "3", "0.707", 3},
		// Odd weights (float to int scaling: 0.6f * 1000 is not exactly 600) and a single postfilter pass.
		{"odd_weights", true, "0.6", "1.5", "4", "0.85", 1},
		// No postfilter at all.
		{"no_postfilter", false, "0.5", "2", "3", "0.707", 0}
	};

	private GeoPathVectors()
	{
	}

	/**
	 * Writes {@code geo_path_download.json} and {@code geo_path_converted.json}.
	 * @param outDir the output folder
	 * @param downloadDir folder with the downloaded {@code .l2j} files
	 * @param convertedDir folder with the converted {@code .l2j} files
	 * @throws Exception if the engine fails
	 */
	public static void generate(Path outDir, Path downloadDir, Path convertedDir) throws Exception
	{
		check(!Files.exists(Path.of("data", "Doors.xml")) && !Files.exists(Path.of("data", "FenceData.xml")), "run from a folder without ./data/Doors.xml or ./data/FenceData.xml (doors/fences must stay empty)");
		GeoEngineConfig.PATHFINDING = 2;
		GeoEngineConfig.AVOID_OBSTRUCTED_PATH_NODES = true;
		GeoEngineConfig.PATHFIND_BUFFERS = BUFFERS;

		// First set: the singleton loads it on first use.
		GeoEngineConfig.GEODATA_PATH = downloadDir;
		final GeoEngine engine = GeoEngine.getInstance();
		generateSet(outDir, "download", downloadDir, engine);

		// Second set: load again through the constructor (static region table).
		GeoEngineConfig.GEODATA_PATH = convertedDir;
		final Constructor<GeoEngine> constructor = GeoEngine.class.getDeclaredConstructor();
		constructor.setAccessible(true);
		constructor.newInstance();
		generateSet(outDir, "converted", convertedDir, engine);
	}

	private static void generateSet(Path outDir, String set, Path geoDir, GeoEngine engine) throws Exception
	{
		final List<int[]> regions = new ArrayList<>();
		for (int[] r : REGIONS)
		{
			regions.add(new int[] {r[0], r[1], 1});
		}
		int missing = 0;
		for (int x = 11; (x <= 26) && (missing < MISSING_REGIONS); x++)
		{
			for (int y = 10; (y <= 26) && (missing < MISSING_REGIONS); y++)
			{
				if (!Files.exists(geoDir.resolve(x + "_" + y + ".l2j")))
				{
					regions.add(new int[] {x, y, 0});
					missing++;
				}
			}
		}
		check(missing == MISSING_REGIONS, "not enough tile positions without a file");

		final List<Object> variantList = new ArrayList<>();
		final int[] totals = new int[3];
		for (int v = 0; v < VARIANTS.length; v++)
		{
			final Object[] cfg = VARIANTS[v];
			GeoEngineConfig.ADVANCED_DIAGONAL_STRATEGY = (Boolean) cfg[1];
			GeoEngineConfig.LOW_WEIGHT = Float.parseFloat((String) cfg[2]);
			GeoEngineConfig.MEDIUM_WEIGHT = Float.parseFloat((String) cfg[3]);
			GeoEngineConfig.HIGH_WEIGHT = Float.parseFloat((String) cfg[4]);
			GeoEngineConfig.DIAGONAL_WEIGHT = Float.parseFloat((String) cfg[5]);
			GeoEngineConfig.MAX_POSTFILTER_PASSES = (Integer) cfg[6];
			// A fresh PathFinding so the thread-local NodeBuffers pick up the weights above.
			final Constructor<PathFinding> constructor = PathFinding.class.getDeclaredConstructor();
			constructor.setAccessible(true);
			final PathFinding pathFinding = constructor.newInstance();

			final List<Object> cases = new ArrayList<>();
			final int perRegion = v == 0 ? BASE_CASES : VARIANT_CASES;
			for (int[] region : regions)
			{
				final int rx = region[0];
				final int ry = region[1];
				final Sampler s = new Sampler(engine, rx, ry, new Random(SEED + (v * 1_000_003L) + (rx * 100L) + ry));
				final int count = region[2] == 1 ? perRegion : 6;
				for (int i = 0; i < count; i++)
				{
					final int[] q = region[2] == 1 ? s.pair() : s.pairInMissing();
					cases.add(run(pathFinding, q, totals));
				}
				if (region[2] == 1)
				{
					// Blocked cells as target (unreachable), and a start / target on the very same cell and on neighbours.
					for (int i = 0; i < 2; i++)
					{
						final int[] q = s.pairToBlockedCell();
						if (q != null)
						{
							cases.add(run(pathFinding, q, totals));
						}
					}
					for (int i = 0; i < 3; i++)
					{
						cases.add(run(pathFinding, s.pairNear(i), totals));
					}
				}
			}

			final Map<String, Object> variant = Json.obj();
			variant.put("name", cfg[0]);
			variant.put("advanced_diagonal_strategy", cfg[1]);
			variant.put("low_weight", cfg[2]);
			variant.put("medium_weight", cfg[3]);
			variant.put("high_weight", cfg[4]);
			variant.put("diagonal_weight", cfg[5]);
			variant.put("max_postfilter_passes", cfg[6]);
			variant.put("cases", cases);
			variantList.add(variant);
		}
		System.out.println("geo_path_" + set + ": found " + totals[0] + ", null " + totals[1] + ", empty " + totals[2]);
		check((totals[0] > 100) && (totals[1] > 20), "path results are one-sided");

		final Map<String, Object> root = Json.header("l2jmobius PathFinding.findPath (NodeBuffer A*, GeoLocation, LOS postfilter) on the whole-world GeoEngine of the set");
		root.put("note", "Cases are strings 'x y z tx ty tz instance playable > result', world coordinates, playable 0/1. result is 'null' (no path, no geodata, or no buffer large enough) or '<count> nodeX,nodeY,geoHeight,nswe ...' (GeoLocation.getNodeX/getNodeY/getZ/packed NSWE, in path order; getX/getY are the world middle of the node cell). Each variant sets the GeoEngineConfig pathfinding statics (weights are strings parsed as Java floats) and uses a fresh PathFinding; PathFinding = 2, AvoidObstructedPathNodes = true, PathFindBuffers = " + BUFFERS + ". Doors and fences: none. Start points are random cells of each region (70% preference for non-flat blocks, 20% near a region border); targets 1..350 cells away in a random direction (60% within 30, 25% within 100, 12% within 200, 3% beyond); extra cases: target on a completely blocked cell, same cell, neighbour cells, start in a region without a file. seed = " + SEED + " + variant * 1000003 + rx * 100 + ry.");
		root.put("set", set);
		root.put("seed", SEED);
		root.put("buffers", BUFFERS);
		root.put("variants", variantList);
		Json.write(outDir.resolve("geo_path_" + set + ".json"), root);
	}

	private static String run(PathFinding pathFinding, int[] q, int[] totals)
	{
		final boolean playable = q[7] == 1;
		final List<GeoLocation> path = pathFinding.findPath(q[0], q[1], q[2], q[3], q[4], q[5], q[6], playable);
		final StringBuilder sb = new StringBuilder();
		for (int i = 0; i < q.length; i++)
		{
			sb.append(q[i]).append(' ');
		}
		sb.append("> ");
		if (path == null)
		{
			sb.append("null");
			totals[1]++;
		}
		else
		{
			sb.append(path.size());
			for (GeoLocation l : path)
			{
				check(l.getX() == GeoEngine.getWorldX(l.getNodeX()) && l.getY() == GeoEngine.getWorldY(l.getNodeY()), "GeoLocation world coordinates");
				sb.append(' ').append(l.getNodeX()).append(',').append(l.getNodeY()).append(',').append(l.getZ()).append(',').append(nsweOf(l));
			}
			totals[path.isEmpty() ? 2 : 0]++;
		}
		return sb.toString();
	}

	/** Packed NSWE of a GeoLocation through its public can-go methods (E=1, W=2, S=4, N=8, like Cell). */
	private static int nsweOf(GeoLocation l)
	{
		return (l.canGoEast() ? 1 : 0) | (l.canGoWest() ? 2 : 0) | (l.canGoSouth() ? 4 : 0) | (l.canGoNorth() ? 8 : 0);
	}

	/** Random start points and start / target pairs inside one region. */
	private static final class Sampler
	{
		private final GeoEngine _engine;
		private final int _regionX;
		private final int _regionY;
		private final Random _random;

		Sampler(GeoEngine engine, int regionX, int regionY, Random random)
		{
			_engine = engine;
			_regionX = regionX;
			_regionY = regionY;
			_random = random;
		}

		private int[] randomCell() throws Exception
		{
			int geoX = 0;
			int geoY = 0;
			for (int tries = 0; tries < 12; tries++)
			{
				geoX = (_regionX * IRegion.REGION_CELLS_X) + _random.nextInt(IRegion.REGION_CELLS_X);
				geoY = (_regionY * IRegion.REGION_CELLS_Y) + _random.nextInt(IRegion.REGION_CELLS_Y);
				if (_random.nextInt(5) == 0)
				{
					final int edge = _random.nextBoolean() ? _random.nextInt(24) : IRegion.REGION_CELLS_X - 1 - _random.nextInt(24);
					if (_random.nextBoolean())
					{
						geoX = (_regionX * IRegion.REGION_CELLS_X) + edge;
					}
					else
					{
						geoY = (_regionY * IRegion.REGION_CELLS_Y) + edge;
					}
				}
				if (!isFlat(geoX, geoY) || (_random.nextInt(100) >= 70))
				{
					break;
				}
			}
			return new int[] {geoX, geoY};
		}

		/** A world point {x, y, z} on the given cell, z near a layer of it. */
		private int[] pointOn(int geoX, int geoY, Integer around)
		{
			final int worldX = (GeoEngine.getWorldX(geoX) - 8) + _random.nextInt(16);
			final int worldY = (GeoEngine.getWorldY(geoY) - 8) + _random.nextInt(16);
			return new int[] {worldX, worldY, zAt(geoX, geoY, around)};
		}

		/** {x, y, z, tx, ty, tz, instance, playable}: a start point and a target some distance away (any direction). */
		int[] pair() throws Exception
		{
			final int[] cell = randomCell();
			final int[] a = pointOn(cell[0], cell[1], null);
			final int roll = _random.nextInt(100);
			final int cells;
			if (roll < 60)
			{
				cells = _random.nextInt(30) + 1;
			}
			else if (roll < 85)
			{
				cells = _random.nextInt(100) + 1;
			}
			else if (roll < 97)
			{
				cells = _random.nextInt(200) + 1;
			}
			else
			{
				cells = _random.nextInt(350) + 1;
			}
			final double angle = _random.nextDouble() * 2 * Math.PI;
			final int tx = a[0] + (int) Math.round(cells * 16 * StrictMath.cos(angle));
			final int ty = a[1] + (int) Math.round(cells * 16 * StrictMath.sin(angle));
			final int tz = zAt(GeoEngine.getGeoX(tx), GeoEngine.getGeoY(ty), a[2]);
			return new int[] {a[0], a[1], a[2], tx, ty, tz, 0, _random.nextInt(4) == 0 ? 0 : 1};
		}

		/** A start somewhere in a region without a file; the target is somewhere near. */
		int[] pairInMissing()
		{
			final int geoX = (_regionX * IRegion.REGION_CELLS_X) + _random.nextInt(IRegion.REGION_CELLS_X);
			final int geoY = (_regionY * IRegion.REGION_CELLS_Y) + _random.nextInt(IRegion.REGION_CELLS_Y);
			final int[] a = pointOn(geoX, geoY, null);
			return new int[] {a[0], a[1], a[2], a[0] + 500, a[1] - 300, a[2], 0, 1};
		}

		/** A target on a completely blocked cell within 40 cells of a start; null when none is found. */
		int[] pairToBlockedCell() throws Exception
		{
			for (int tries = 0; tries < 400; tries++)
			{
				final int[] cell = randomCell();
				final int[] a = pointOn(cell[0], cell[1], null);
				final int gx = cell[0] + (_random.nextInt(81) - 40);
				final int gy = cell[1] + (_random.nextInt(81) - 40);
				final int gz = _engine.getNearestZ(gx, gy, a[2]);
				if (_engine.hasGeoPos(gx, gy) && _engine.isCompletelyBlocked(gx, gy, gz))
				{
					final int[] t = pointOn(gx, gy, a[2]);
					return new int[] {a[0], a[1], a[2], t[0], t[1], gz, 0, 1};
				}
			}
			return null;
		}

		/** 0: the same cell, 1: the next cell east, 2: a cell diagonally (southeast). */
		int[] pairNear(int kind) throws Exception
		{
			final int[] cell = randomCell();
			final int[] a = pointOn(cell[0], cell[1], null);
			final int dx = kind == 0 ? 0 : 1;
			final int dy = kind == 2 ? 1 : 0;
			final int[] t = pointOn(cell[0] + dx, cell[1] + dy, a[2]);
			return new int[] {a[0], a[1], a[2], t[0], t[1], t[2], 0, 1};
		}

		/** A z close to a layer of the cell: nearest layer to {@code around} (or a random height), plus a small offset; 5% fully random. */
		private int zAt(int geoX, int geoY, Integer around)
		{
			if (_random.nextInt(100) < 5)
			{
				return _random.nextInt(32001) - 16000;
			}
			final int probe = around != null ? around + (_random.nextInt(121) - 60) : _random.nextInt(8001) - 4000;
			final int[] offsets = {0, 0, 0, 0, 8, 24, -24, 70};
			return _engine.getNearestZ(geoX, geoY, probe) + offsets[_random.nextInt(offsets.length)];
		}

		private boolean isFlat(int geoX, int geoY) throws Exception
		{
			final IRegion region = _engine.getRegion(geoX, geoY);
			if (!(region instanceof Region))
			{
				return true;
			}
			final Field f = Region.class.getDeclaredField("_blocks");
			f.setAccessible(true);
			final IBlock[] blocks = (IBlock[]) f.get(region);
			return blocks[(((geoX / IBlock.BLOCK_CELLS_X) % IRegion.REGION_BLOCKS_X) * IRegion.REGION_BLOCKS_Y) + ((geoY / IBlock.BLOCK_CELLS_Y) % IRegion.REGION_BLOCKS_Y)] instanceof FlatBlock;
		}
	}

	private static void check(boolean ok, String message)
	{
		if (!ok)
		{
			throw new IllegalStateException("vector self-check failed: " + message);
		}
	}
}
