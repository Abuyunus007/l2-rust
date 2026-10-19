package org.l2jmobius.gameserver.managers;

import java.util.concurrent.atomic.AtomicInteger;

/**
 * Java-vectors harness replacement of {@code org.l2jmobius.gameserver.managers.IdManager}.<br>
 * The real class cleans and scans the database in its constructor; the vector generators must never open a database connection.
 * This stub hands out ascending ids from the same first id and has the same public API. It sits on the class path before the L2JMobius sources
 * (the Java tree is not modified).
 */
public class IdManager
{
	private static final IdManager INSTANCE = new IdManager();

	private final AtomicInteger _next = new AtomicInteger(0x10000000);

	public int getNextId()
	{
		return _next.getAndIncrement();
	}

	public void releaseId(int objectId)
	{
	}

	public int getAvailableIdCount()
	{
		return Integer.MAX_VALUE;
	}

	public static IdManager getInstance()
	{
		return INSTANCE;
	}
}
