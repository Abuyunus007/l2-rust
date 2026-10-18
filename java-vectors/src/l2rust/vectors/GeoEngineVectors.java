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
import org.l2jmobius.gameserver.geoengine.geodata.Cell;
import org.l2jmobius.gameserver.geoengine.geodata.IBlock;
import org.l2jmobius.gameserver.geoengine.geodata.IRegion;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.FlatBlock;
import org.l2jmobius.gameserver.geoengine.geodata.regions.Region;
import org.l2jmobius.gameserver.geoengine.util.GridLineIterator2D;
import org.l2jmobius.gameserver.geoengine.util.GridLineIterator3D;

/**
 * Vectors for the {@code GeoEngine} port (M3-GEO-01). Every expected value comes from the real L2JMobius {@link GeoEngine}, {@link GridLineIterator2D} and {@link GridLineIterator3D}.
 * <p>
 * How the engine is driven:
 * <ul>
 * <li>{@code GeoEngine}'s constructor is {@code protected} and loads the whole world from {@code GeoEngineConfig.GEODATA_PATH}. The harness sets that public static field, then calls the constructor by reflection (once per set; the constructor resets the static region table first).</li>
 * <li>{@code GeoEngineConfig.PATHFINDING} and {@code AVOID_OBSTRUCTED_PATH_NODES} are public statics too and are set directly ({@code GeoEngineConfig.load()} and its ini file are not used). Default for the main arrays: the shipped {@code GeoEngine.ini} values, 2 and true.</li>
 * <li><b>Doors and fences are not stubbed.</b> {@code DoorData} and {@code FenceData} are the real singletons; their {@code load()} finds no {@code ./data/Doors.xml} / {@code FenceData.xml} relative to the working directory (log warning only) and {@code World.init()} is never called, so {@code World.getRegion} answers null. {@code checkIfDoorsBetween} / {@code checkIfFenceBetween} therefore return false: the "no obstacles" behaviour. The run aborts if the working directory has those files.</li>
 * <li>Only instance id 0 is used (a non-zero id would go through {@code InstanceManager}).</li>
 * </ul>
 * Rules (same as {@link GeoVectors}): no clock, no unseeded randomness; all sample points come from {@code new Random(seed)} and are written into the JSON.
 * <p>
 * Case format: every case is one string {@code "<inputs> > <outputs>"}, integers separated by single spaces (compact: one line per case). Booleans are 0/1. Points in the line iterator files are {@code x,y} or {@code x,y,z}.
 */
public final class GeoEngineVectors
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

	private static final long SEED = 20260418L;

	private static final int HEIGHT_CASES = 120;
	private static final int NSWE_CASES = 80;
	private static final int LOS_CASES = 220;
	private static final int MOVE_CASES = 150;
	private static final int VALID_CASES = 150;
	/** Cases per region for the alternative config variants (move + valid). */
	private static final int VARIANT_CASES = 50;

	/** Directions probed by the anti-corner-cut check, in this order (bit i of the result = entry i). */
	private static final int[] NSWE_PROBES =
	{
		Cell.NSWE_EAST,
		Cell.NSWE_WEST,
		Cell.NSWE_SOUTH,
		Cell.NSWE_NORTH,
		Cell.NSWE_NORTH_EAST,
		Cell.NSWE_NORTH_WEST,
		Cell.NSWE_SOUTH_EAST,
		Cell.NSWE_SOUTH_WEST
	};

	/** {pathfinding, avoidObstructedPathNodes}: shipped config first, then one flag flipped at a time. */
	private static final int[][] CONFIG_VARIANTS =
	{
		{0, 1},
		{2, 0}
	};

	private GeoEngineVectors()
	{
	}

	/**
	 * Writes {@code geo_engine_coords.json} (no geodata needed), {@code geo_engine_download.json} and {@code geo_engine_converted.json}.
	 * @param outDir the output folder
	 * @param downloadDir folder with the downloaded {@code .l2j} files
	 * @param convertedDir folder with the converted {@code .l2j} files
	 * @throws Exception if the engine fails
	 */
	public static void generate(Path outDir, Path downloadDir, Path convertedDir) throws Exception
	{
		check(!Files.exists(Path.of("data", "Doors.xml")) && !Files.exists(Path.of("data", "FenceData.xml")), "run from a folder without ./data/Doors.xml or ./data/FenceData.xml (doors/fences must stay empty)");
		generateCoords(outDir);
		generateSet(outDir, "download", downloadDir);
		generateSet(outDir, "converted", convertedDir);
	}

	// ---------------------------------------------------------------- coordinates and line iterators

	private static void generateCoords(Path outDir) throws Exception
	{
		final Random random = new Random(SEED);
		final int worldMinX = -655360;
		final int worldMinY = -589824;

		final List<Object> geoX = new ArrayList<>();
		final List<Object> geoY = new ArrayList<>();
		final int[] edgeWorld =
		{
			Integer.MIN_VALUE, -2000000000, -1000000, -700000, worldMinX - 17, worldMinX - 16, worldMinX - 15, worldMinX - 2, worldMinX - 1, worldMinX, worldMinX + 1, worldMinX + 15, worldMinX + 16, worldMinX + 17,
			worldMinY - 17, worldMinY - 1, worldMinY, worldMinY + 15, worldMinY + 16,
			-17, -16, -15, -9, -8, -7, -1, 0, 1, 7, 8, 9, 15, 16, 17, 32767, 32768, 32769, 147000, 229375, 229376, 294911, 294912, 1000000, 1073741823, 2000000000, Integer.MAX_VALUE
		};
		final List<Integer> worldValues = new ArrayList<>();
		for (int v : edgeWorld)
		{
			worldValues.add(v);
		}
		for (int i = 0; i < 300; i++)
		{
			worldValues.add(random.nextInt(1_300_000) - 800_000);
		}
		for (int v : worldValues)
		{
			geoX.add(v + " > " + GeoEngine.getGeoX(v));
			geoY.add(v + " > " + GeoEngine.getGeoY(v));
		}

		final List<Object> worldX = new ArrayList<>();
		final List<Object> worldY = new ArrayList<>();
		final int[] edgeGeo =
		{
			Integer.MIN_VALUE, -134217728, -100000, -5000, -2049, -2048, -2047, -17, -2, -1, 0, 1, 2, 7, 8, 9, 2047, 2048, 2049, 4095, 4096, 55295, 55296, 65535, 65536, 70000, 134217727, 134217728, Integer.MAX_VALUE
		};
		final List<Integer> geoValues = new ArrayList<>();
		for (int v : edgeGeo)
		{
			geoValues.add(v);
		}
		for (int i = 0; i < 300; i++)
		{
			geoValues.add(random.nextInt(75_000) - 5_000);
		}
		for (int v : geoValues)
		{
			worldX.add(v + " > " + GeoEngine.getWorldX(v));
			worldY.add(v + " > " + GeoEngine.getWorldY(v));
		}

		final List<Object> line2d = new ArrayList<>();
		final List<Object> line3d = new ArrayList<>();
		final List<int[]> fixed = new ArrayList<>();
		// start x, y, z, end x, y, z: same point, axis aligned, diagonals, equal deltas, deltas of 1, negative coordinates.
		fixed.add(new int[] {0, 0, 0, 0, 0, 0});
		fixed.add(new int[] {5, 5, 5, 5, 5, 5});
		fixed.add(new int[] {0, 0, 0, 10, 0, 0});
		fixed.add(new int[] {0, 0, 0, -10, 0, 0});
		fixed.add(new int[] {0, 0, 0, 0, 10, 0});
		fixed.add(new int[] {0, 0, 0, 0, -10, 0});
		fixed.add(new int[] {0, 0, 0, 0, 0, 10});
		fixed.add(new int[] {0, 0, 0, 0, 0, -10});
		fixed.add(new int[] {0, 0, 0, 7, 7, 7});
		fixed.add(new int[] {0, 0, 0, -7, -7, -7});
		fixed.add(new int[] {0, 0, 0, 7, -7, 7});
		fixed.add(new int[] {0, 0, 0, 10, 10, 3});
		fixed.add(new int[] {0, 0, 0, 3, 10, 10});
		fixed.add(new int[] {0, 0, 0, 10, 3, 10});
		fixed.add(new int[] {0, 0, 0, 1, 1, 1});
		fixed.add(new int[] {0, 0, 0, 1, 0, 0});
		fixed.add(new int[] {-3, -3, -3, 3, 3, 3});
		fixed.add(new int[] {-20, -15, 4, 12, 9, -6});
		fixed.add(new int[] {-1, -1, 0, -9, -4, 0});
		fixed.add(new int[] {100, 200, 300, 91, 207, 255});
		for (int[] f : fixed)
		{
			line2d.add(line2d(f[0], f[1], f[3], f[4]));
			line3d.add(line3d(f[0], f[1], f[2], f[3], f[4], f[5]));
		}
		for (int i = 0; i < 150; i++)
		{
			final int sx = random.nextInt(101) - 50;
			final int sy = random.nextInt(101) - 50;
			final int sz = random.nextInt(101) - 50;
			// Equal deltas and ties are more likely with small ranges; mix small and large.
			final int range = i % 3 == 0 ? 6 : 60;
			final int ex = sx + (random.nextInt((2 * range) + 1) - range);
			final int ey = sy + (random.nextInt((2 * range) + 1) - range);
			final int ez = sz + (random.nextInt((2 * range) + 1) - range);
			line2d.add(line2d(sx, sy, ex, ey));
			line3d.add(line3d(sx, sy, sz, ex, ey, ez));
		}

		final Map<String, Object> root = Json.header("l2jmobius GeoEngine.getGeoX/getGeoY/getWorldX/getWorldY (static), GridLineIterator2D, GridLineIterator3D");
		root.put("note", "Cases are strings '<inputs> > <outputs>', integers separated by spaces. geo_x/geo_y: 'worldValue > geoValue'; world_x/world_y: 'geoValue > worldValue' (int overflow wraps like Java). line2d: 'sx sy ex ey > x,y x,y ...' and line3d: 'sx sy sz ex ey ez > x,y,z ...' list every point the iterator visits, start and end included.");
		root.put("seed", SEED);
		root.put("geo_x", geoX);
		root.put("geo_y", geoY);
		root.put("world_x", worldX);
		root.put("world_y", worldY);
		root.put("line2d", line2d);
		root.put("line3d", line3d);
		Json.write(outDir.resolve("geo_engine_coords.json"), root);
	}

	private static String line2d(int sx, int sy, int ex, int ey)
	{
		final StringBuilder sb = new StringBuilder();
		sb.append(sx).append(' ').append(sy).append(' ').append(ex).append(' ').append(ey).append(" >");
		final GridLineIterator2D it = new GridLineIterator2D(sx, sy, ex, ey);
		int guard = 0;
		while (it.next())
		{
			sb.append(' ').append(it.x()).append(',').append(it.y());
			check(++guard < 10_000, "line2d does not terminate");
		}
		return sb.toString();
	}

	private static String line3d(int sx, int sy, int sz, int ex, int ey, int ez)
	{
		final StringBuilder sb = new StringBuilder();
		sb.append(sx).append(' ').append(sy).append(' ').append(sz).append(' ').append(ex).append(' ').append(ey).append(' ').append(ez).append(" >");
		final GridLineIterator3D it = new GridLineIterator3D(sx, sy, sz, ex, ey, ez);
		int guard = 0;
		while (it.next())
		{
			sb.append(' ').append(it.x()).append(',').append(it.y()).append(',').append(it.z());
			check(++guard < 10_000, "line3d does not terminate");
		}
		return sb.toString();
	}

	// ---------------------------------------------------------------- engine queries per set

	private static void generateSet(Path outDir, String set, Path geoDir) throws Exception
	{
		GeoEngineConfig.GEODATA_PATH = geoDir;
		GeoEngineConfig.PATHFINDING = 2;
		GeoEngineConfig.AVOID_OBSTRUCTED_PATH_NODES = true;
		final Constructor<GeoEngine> constructor = GeoEngine.class.getDeclaredConstructor();
		constructor.setAccessible(true);
		final GeoEngine engine = constructor.newInstance();

		// Regions to probe: the six of the analysis doc plus tile positions that have no file.
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

		final List<Object> regionInfo = new ArrayList<>();
		final List<Object> height = new ArrayList<>();
		final List<Object> nswe = new ArrayList<>();
		final List<Object> los = new ArrayList<>();
		final List<Object> move = new ArrayList<>();
		final List<Object> valid = new ArrayList<>();
		final List<Object> variants = new ArrayList<>();
		final List<List<Object>> variantMove = new ArrayList<>();
		final List<List<Object>> variantValid = new ArrayList<>();
		final List<List<Object>> variantBlocked = new ArrayList<>();
		for (int i = 0; i < CONFIG_VARIANTS.length; i++)
		{
			variantMove.add(new ArrayList<>());
			variantValid.add(new ArrayList<>());
			variantBlocked.add(new ArrayList<>());
		}

		final int[] counts = new int[2];
		for (int[] region : regions)
		{
			final int rx = region[0];
			final int ry = region[1];
			final boolean present = region[2] == 1;
			final Sampler s = new Sampler(engine, rx, ry, new Random(SEED + (rx * 100L) + ry));
			regionInfo.add(rx + "_" + ry + " > " + region[2]);
			check(present == engine.hasGeoPos(rx * IRegion.REGION_CELLS_X, ry * IRegion.REGION_CELLS_Y), "region presence " + rx + "_" + ry);

			// getHeight / hasGeo / getSpawnHeight / the geo-coordinate wrappers.
			for (int i = 0; i < HEIGHT_CASES; i++)
			{
				final int[] p = s.start();
				final int gx = GeoEngine.getGeoX(p[0]);
				final int gy = GeoEngine.getGeoY(p[1]);
				height.add(p[0] + " " + p[1] + " " + p[2] + " > " + engine.getHeight(p[0], p[1], p[2]) + " " + b(engine.hasGeo(p[0], p[1])) + " " + engine.getSpawnHeight(p[0], p[1], p[2]) + " " + gx + " " + gy + " " + engine.getNearestZ(gx, gy, p[2]) + " " + engine.getNextLowerZ(gx, gy, p[2]) + " " + engine.getNextHigherZ(gx, gy, p[2]));
			}

			// checkNearestNsweAntiCornerCut for single and diagonal directions, isCompletelyBlocked.
			for (int i = 0; i < NSWE_CASES; i++)
			{
				final int[] p = s.start();
				final int gx = GeoEngine.getGeoX(p[0]);
				final int gy = GeoEngine.getGeoY(p[1]);
				int mask = 0;
				for (int d = 0; d < NSWE_PROBES.length; d++)
				{
					if (engine.checkNearestNsweAntiCornerCut(gx, gy, p[2], NSWE_PROBES[d]))
					{
						mask |= 1 << d;
					}
				}
				nswe.add(gx + " " + gy + " " + p[2] + " > " + mask + " " + b(engine.isCompletelyBlocked(gx, gy, p[2])));
			}

			// canSeeTarget (7 args, instance 0; the 6-arg overload must agree because no doors / fences exist).
			for (int i = 0; i < LOS_CASES; i++)
			{
				final int[] q = s.pair();
				final boolean result = engine.canSeeTarget(q[0], q[1], q[2], q[3], q[4], q[5], 0);
				check(result == engine.canSeeTarget(q[0], q[1], q[2], q[3], q[4], q[5]), "canSeeTarget overloads differ");
				los.add(join(q) + " 0 > " + b(result));
				counts[result ? 1 : 0]++;
			}

			// canMoveToTarget and getValidLocation with the shipped config.
			for (int i = 0; i < MOVE_CASES; i++)
			{
				final int[] q = s.pair();
				move.add(join(q) + " 0 > " + b(engine.canMoveToTarget(q[0], q[1], q[2], q[3], q[4], q[5], 0)));
			}
			for (int i = 0; i < VALID_CASES; i++)
			{
				final int[] q = s.pair();
				final var loc = engine.getValidLocation(q[0], q[1], q[2], q[3], q[4], q[5], 0);
				valid.add(join(q) + " 0 > " + loc.getX() + " " + loc.getY() + " " + loc.getZ());
			}

			// The same queries with one config flag flipped.
			for (int v = 0; v < CONFIG_VARIANTS.length; v++)
			{
				GeoEngineConfig.PATHFINDING = CONFIG_VARIANTS[v][0];
				GeoEngineConfig.AVOID_OBSTRUCTED_PATH_NODES = CONFIG_VARIANTS[v][1] == 1;
				for (int i = 0; i < VARIANT_CASES; i++)
				{
					final int[] q = s.pair();
					variantMove.get(v).add(join(q) + " 0 > " + b(engine.canMoveToTarget(q[0], q[1], q[2], q[3], q[4], q[5], 0)));
					final int[] w = s.pair();
					final var loc = engine.getValidLocation(w[0], w[1], w[2], w[3], w[4], w[5], 0);
					variantValid.get(v).add(join(w) + " 0 > " + loc.getX() + " " + loc.getY() + " " + loc.getZ());
					final int[] p = s.start();
					final int gx = GeoEngine.getGeoX(p[0]);
					final int gy = GeoEngine.getGeoY(p[1]);
					variantBlocked.get(v).add(gx + " " + gy + " " + p[2] + " > " + b(engine.isCompletelyBlocked(gx, gy, p[2])));
				}
				GeoEngineConfig.PATHFINDING = 2;
				GeoEngineConfig.AVOID_OBSTRUCTED_PATH_NODES = true;
			}
		}
		// Make sure both outcomes are exercised, otherwise the vectors prove little.
		check((counts[0] > 100) && (counts[1] > 100), "line of sight results are one-sided: blocked " + counts[0] + ", visible " + counts[1]);

		for (int v = 0; v < CONFIG_VARIANTS.length; v++)
		{
			final Map<String, Object> variant = Json.obj();
			variant.put("pathfinding", CONFIG_VARIANTS[v][0]);
			variant.put("avoid_obstructed_path_nodes", CONFIG_VARIANTS[v][1] == 1);
			variant.put("can_move", variantMove.get(v));
			variant.put("valid_location", variantValid.get(v));
			variant.put("completely_blocked", variantBlocked.get(v));
			variants.add(variant);
		}

		final Map<String, Object> root = Json.header("l2jmobius GeoEngine (whole-world constructor, GEODATA_PATH = the set folder): getHeight, hasGeo, getSpawnHeight, getNearestZ/getNextLowerZ/getNextHigherZ, checkNearestNsweAntiCornerCut, isCompletelyBlocked, canSeeTarget, canMoveToTarget, getValidLocation");
		root.put("note", "Cases are strings '<inputs> > <outputs>', integers separated by spaces, booleans 0/1. regions: 'rx_ry > present' (0 = no file: NullRegion). height: 'x y z > getHeight hasGeo getSpawnHeight geoX geoY nearestZ nextLowerZ nextHigherZ' (world x y z, geo functions at that world point's cell). nswe: 'geoX geoY z > mask blocked' where bit i of mask is checkNearestNsweAntiCornerCut for E, W, S, N, NE, NW, SE, SW (i = 0..7) and blocked is isCompletelyBlocked. los / can_move: 'x y z tx ty tz instance > result'. valid_location: 'x y z tx ty tz instance > x y z'. All with PathFinding = 2 and AvoidObstructedPathNodes = true (the shipped GeoEngine.ini); variants repeat can_move / valid_location / completely_blocked with one flag flipped. Doors and fences: none (real DoorData / FenceData with no data files). Start points are random cells of each region (70% preference for non-flat blocks, 20% near a region border), z = a layer of the cell plus a small offset, targets within about 5000 units; seed = 20260418 + rx * 100 + ry.");
		root.put("set", set);
		root.put("seed", SEED);
		root.put("regions", regionInfo);
		root.put("height", height);
		root.put("nswe", nswe);
		root.put("los", los);
		root.put("can_move", move);
		root.put("valid_location", valid);
		root.put("variants", variants);
		Json.write(outDir.resolve("geo_engine_" + set + ".json"), root);
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

		/** A world point {x, y, z} on a random cell of the region. */
		int[] start() throws Exception
		{
			int geoX = 0;
			int geoY = 0;
			for (int tries = 0; tries < 12; tries++)
			{
				geoX = (_regionX * IRegion.REGION_CELLS_X) + _random.nextInt(IRegion.REGION_CELLS_X);
				geoY = (_regionY * IRegion.REGION_CELLS_Y) + _random.nextInt(IRegion.REGION_CELLS_Y);
				if (_random.nextInt(5) == 0)
				{
					// Near a region border.
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
			final int worldX = (GeoEngine.getWorldX(geoX) - 8) + _random.nextInt(16);
			final int worldY = (GeoEngine.getWorldY(geoY) - 8) + _random.nextInt(16);
			return new int[] {worldX, worldY, zAt(geoX, geoY, null)};
		}

		/** {x, y, z, tx, ty, tz}: a start point and a target some distance away (any direction, may leave the region). */
		int[] pair() throws Exception
		{
			final int[] a = start();
			final int roll = _random.nextInt(100);
			final double distance;
			if (roll < 60)
			{
				distance = _random.nextInt(300) + 1;
			}
			else if (roll < 92)
			{
				distance = _random.nextInt(1500) + 1;
			}
			else
			{
				distance = _random.nextInt(5000) + 1;
			}
			final double angle = _random.nextDouble() * 2 * Math.PI;
			final int tx = a[0] + (int) Math.round(distance * StrictMath.cos(angle));
			final int ty = a[1] + (int) Math.round(distance * StrictMath.sin(angle));
			final int tz = zAt(GeoEngine.getGeoX(tx), GeoEngine.getGeoY(ty), a[2]);
			return new int[] {a[0], a[1], a[2], tx, ty, tz};
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

	private static String join(int[] q)
	{
		final StringBuilder sb = new StringBuilder();
		for (int i = 0; i < q.length; i++)
		{
			if (i > 0)
			{
				sb.append(' ');
			}
			sb.append(q[i]);
		}
		return sb.toString();
	}

	private static int b(boolean value)
	{
		return value ? 1 : 0;
	}

	private static void check(boolean ok, String message)
	{
		if (!ok)
		{
			throw new IllegalStateException("vector self-check failed: " + message);
		}
	}
}
