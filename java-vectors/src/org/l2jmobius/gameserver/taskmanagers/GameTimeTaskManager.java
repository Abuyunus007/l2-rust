package org.l2jmobius.gameserver.taskmanagers;

/**
 * Java-vectors harness replacement of {@code org.l2jmobius.gameserver.taskmanagers.GameTimeTaskManager} (same public API).<br>
 * The real class starts a thread that derives the game time and the day/night flag from the wall clock, which would make the hit chance (night malus of
 * {@code HitConditionBonusData}) depend on the time of the run. This stub has no thread and no clock: the game time is fixed and the night flag is set by the generator
 * ({@link #setNight(boolean)}).
 */
public class GameTimeTaskManager extends Thread
{
	public static final int TICKS_PER_SECOND = 10; // Not able to change this without checking through code.
	public static final int MILLIS_IN_TICK = 1000 / TICKS_PER_SECOND;
	public static final int IG_DAYS_PER_DAY = 6;
	public static final int MILLIS_PER_IG_DAY = (3600000 * 24) / IG_DAYS_PER_DAY;
	public static final int SECONDS_PER_IG_DAY = MILLIS_PER_IG_DAY / 1000;
	public static final int TICKS_PER_IG_DAY = SECONDS_PER_IG_DAY * TICKS_PER_SECOND;

	private static final GameTimeTaskManager INSTANCE = new GameTimeTaskManager();

	private volatile boolean _isNight = false;

	protected GameTimeTaskManager()
	{
		super("GameTimeTaskManager");
		super.setDaemon(true);
	}

	/** Harness only: forces the day/night flag. */
	public void setNight(boolean night)
	{
		_isNight = night;
	}

	public boolean isNight()
	{
		return _isNight;
	}

	/** @return a fixed tick count: 12:00 real time since midnight, 12 * 3600 s * 10 ticks per second */
	public int getGameTicks()
	{
		return 12 * 3600 * TICKS_PER_SECOND;
	}

	public int getGameTime()
	{
		return 12 * 60;
	}

	public int getGameHour()
	{
		return 12;
	}

	public int getGameMinute()
	{
		return 0;
	}

	public static GameTimeTaskManager getInstance()
	{
		return INSTANCE;
	}
}
